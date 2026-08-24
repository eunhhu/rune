use std::{convert::Infallible, env, fs, process::ExitCode};

use spellwire_core::{
    key, Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent, Program,
    Runtime, RuntimeConfig, Trigger, VmScratch,
};

#[derive(Default)]
struct RecordingInjector {
    batches: Vec<Vec<OutputEvent>>,
}

impl RecordingInjector {
    fn clear(&mut self) {
        self.batches.clear();
    }
}

impl Injector for RecordingInjector {
    type Error = Infallible;

    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
        self.batches.push(events.to_vec());
        Ok(())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}\n");
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    match command.as_str() {
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        "inspect" => {
            let path =
                args.next().ok_or_else(|| "inspect requires a .spellwire.bin path".to_owned())?;
            if args.next().is_some() {
                return Err("inspect accepts exactly one program path".to_owned());
            }
            inspect(&path)
        }
        "simulate" => {
            let path =
                args.next().ok_or_else(|| "simulate requires a .spellwire.bin path".to_owned())?;
            let event_specs = args.collect::<Vec<_>>();
            simulate(&path, &event_specs)
        }
        _ => Err(format!("unknown command {command:?}")),
    }
}

fn load_program(path: &str) -> Result<Program, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {path}: {error}"))?;
    Program::decode(&bytes).map_err(|error| format!("failed to decode {path}: {error}"))
}

fn inspect(path: &str) -> Result<(), String> {
    let program = load_program(path)?;
    println!("program: {path}");
    println!("handlers: {}", program.handlers.len());
    println!("persistent states: {}", program.initial_state.len());
    println!("instructions: {}", program.code.len());
    println!("locals: {}", program.local_count);
    println!("stack limit: {}", program.stack_limit);
    println!("instruction budget: {}", program.instruction_budget);
    println!("initial state: {:?}", program.initial_state);

    for (index, handler) in program.handlers.iter().enumerate() {
        println!("handler[{index}]: {} -> pc {}", format_trigger(handler.trigger), handler.entry);
    }
    Ok(())
}

fn simulate(path: &str, event_specs: &[String]) -> Result<(), String> {
    if event_specs.is_empty() {
        return Err("simulate requires at least one event, for example key-down:Q".to_owned());
    }

    let program = load_program(path)?;
    let mut runtime = Runtime::new(program, RuntimeConfig::default())
        .map_err(|error| format!("program validation failed: {error}"))?;
    let mut scratch = VmScratch::new();
    let mut injector = RecordingInjector::default();

    println!("simulating {} event(s) with {path}", event_specs.len());
    println!("initial state: {:?}", runtime.state());

    for (index, event_spec) in event_specs.iter().enumerate() {
        let event = parse_event(event_spec)?;
        injector.clear();
        let report = runtime
            .dispatch(event, &mut injector, &mut scratch)
            .map_err(|error| format!("dispatch failed for {event_spec:?}: {error:?}"))?;

        println!(
            "\n#{:02} {} | handlers={} instructions={} outputs={}",
            index + 1,
            format_input(event),
            report.handlers,
            report.instructions,
            report.output_events,
        );

        if injector.batches.is_empty() {
            println!("  output: (none)");
        } else {
            for (batch_index, batch) in injector.batches.iter().enumerate() {
                let rendered = batch.iter().map(format_output).collect::<Vec<_>>().join(", ");
                println!("  batch[{batch_index}]: {rendered}");
            }
        }
        println!("  state: {:?}", runtime.state());
    }

    Ok(())
}

fn parse_event(raw: &str) -> Result<InputEvent, String> {
    let mut parts = raw.split(':');
    let kind = parts.next().unwrap_or_default().to_ascii_lowercase();
    let code = parts
        .next()
        .ok_or_else(|| format!("event {raw:?} needs a code, for example key-down:Q"))?;
    let source = parts.next().map(parse_source).transpose()?.unwrap_or(InputSource::Physical);
    if parts.next().is_some() {
        return Err(format!("event {raw:?} has too many ':' fields"));
    }

    let (device, edge, code) = match kind.as_str() {
        "key-down" | "keydown" => (InputDevice::Keyboard, Edge::Down, parse_key(code)?),
        "key-up" | "keyup" => (InputDevice::Keyboard, Edge::Up, parse_key(code)?),
        "mouse-down" | "mousedown" => {
            (InputDevice::MouseButton, Edge::Down, parse_mouse_button(code)? as u16)
        }
        "mouse-up" | "mouseup" => {
            (InputDevice::MouseButton, Edge::Up, parse_mouse_button(code)? as u16)
        }
        _ => {
            return Err(format!(
                "unknown event kind {kind:?}; use key-down, key-up, mouse-down, or mouse-up"
            ));
        }
    };

    Ok(InputEvent { device, code, edge, source })
}

fn parse_source(raw: &str) -> Result<InputSource, String> {
    match normalize(raw).as_str() {
        "physical" | "hardware" => Ok(InputSource::Physical),
        "synthetic" | "injected" => Ok(InputSource::Synthetic),
        _ => Err(format!("unknown event source {raw:?}; use physical or synthetic")),
    }
}

fn parse_key(raw: &str) -> Result<u16, String> {
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        return u16::from_str_radix(hex, 16)
            .map_err(|_| format!("invalid hexadecimal key code {raw:?}"));
    }
    if raw.len() > 1 && raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return raw.parse::<u16>().map_err(|_| format!("invalid key code {raw:?}"));
    }

    let name = normalize(raw);
    if name.len() == 1 {
        let byte = name.as_bytes()[0];
        if byte.is_ascii_lowercase() {
            return Ok(key::A + u16::from(byte - b'a'));
        }
        if (b'1'..=b'9').contains(&byte) {
            return Ok(key::DIGIT_1 + u16::from(byte - b'1'));
        }
        if byte == b'0' {
            return Ok(key::DIGIT_0);
        }
    }

    if let Some(number) = name.strip_prefix('f').and_then(|value| value.parse::<u16>().ok()) {
        if (1..=12).contains(&number) {
            return Ok(key::F1 + number - 1);
        }
    }

    let code = match name.as_str() {
        "enter" | "return" => key::ENTER,
        "escape" | "esc" => key::ESCAPE,
        "backspace" => key::BACKSPACE,
        "tab" => key::TAB,
        "space" => key::SPACE,
        "minus" => key::MINUS,
        "equal" | "equals" => key::EQUAL,
        "leftbracket" => key::LEFT_BRACKET,
        "rightbracket" => key::RIGHT_BRACKET,
        "backslash" => key::BACKSLASH,
        "semicolon" => key::SEMICOLON,
        "quote" => key::QUOTE,
        "grave" | "backtick" => key::GRAVE,
        "comma" => key::COMMA,
        "period" | "dot" => key::PERIOD,
        "slash" => key::SLASH,
        "capslock" => key::CAPS_LOCK,
        "printscreen" => key::PRINT_SCREEN,
        "scrolllock" => key::SCROLL_LOCK,
        "pause" | "break" => key::PAUSE,
        "insert" => key::INSERT,
        "home" => key::HOME,
        "pageup" => key::PAGE_UP,
        "delete" | "del" => key::DELETE,
        "end" => key::END,
        "pagedown" => key::PAGE_DOWN,
        "arrowright" | "right" => key::ARROW_RIGHT,
        "arrowleft" | "left" => key::ARROW_LEFT,
        "arrowdown" | "down" => key::ARROW_DOWN,
        "arrowup" | "up" => key::ARROW_UP,
        "leftcontrol" | "leftctrl" | "lctrl" => key::LEFT_CONTROL,
        "leftshift" | "lshift" => key::LEFT_SHIFT,
        "leftalt" | "lalt" => key::LEFT_ALT,
        "leftmeta" | "leftcommand" | "leftwin" | "lmeta" => key::LEFT_META,
        "rightcontrol" | "rightctrl" | "rctrl" => key::RIGHT_CONTROL,
        "rightshift" | "rshift" => key::RIGHT_SHIFT,
        "rightalt" | "ralt" => key::RIGHT_ALT,
        "rightmeta" | "rightcommand" | "rightwin" | "rmeta" => key::RIGHT_META,
        _ => return Err(format!("unknown key {raw:?}; use a Key name or a 0xNN HID code")),
    };
    Ok(code)
}

fn parse_mouse_button(raw: &str) -> Result<MouseButton, String> {
    let name = normalize(raw);
    match name.as_str() {
        "0" | "left" => Ok(MouseButton::Left),
        "1" | "right" => Ok(MouseButton::Right),
        "2" | "middle" => Ok(MouseButton::Middle),
        "3" | "back" => Ok(MouseButton::Back),
        "4" | "forward" => Ok(MouseButton::Forward),
        _ => {
            Err(format!("unknown mouse button {raw:?}; use left, right, middle, back, or forward"))
        }
    }
}

fn normalize(value: &str) -> String {
    value.chars().filter(char::is_ascii_alphanumeric).flat_map(char::to_lowercase).collect()
}

fn format_trigger(trigger: Trigger) -> String {
    let event = InputEvent {
        device: trigger.device,
        code: trigger.code,
        edge: trigger.edge,
        source: match trigger.source {
            spellwire_core::SourceFilter::Synthetic => InputSource::Synthetic,
            spellwire_core::SourceFilter::Physical | spellwire_core::SourceFilter::Any => {
                InputSource::Physical
            }
        },
    };
    let source = match trigger.source {
        spellwire_core::SourceFilter::Physical => "physical",
        spellwire_core::SourceFilter::Synthetic => "synthetic",
        spellwire_core::SourceFilter::Any => "any",
    };
    format!("{}:{source}", format_input(event))
}

fn format_input(event: InputEvent) -> String {
    let source = match event.source {
        InputSource::Physical => "physical",
        InputSource::Synthetic => "synthetic",
    };
    let edge = if event.edge == Edge::Down { "down" } else { "up" };
    match event.device {
        InputDevice::Keyboard => format!("key-{edge}:{}:{source}", format_key(event.code)),
        InputDevice::MouseButton => {
            format!("mouse-{edge}:{}:{source}", format_mouse_code(event.code))
        }
    }
}

fn format_output(event: &OutputEvent) -> String {
    match *event {
        OutputEvent::Empty => "empty".to_owned(),
        OutputEvent::Key { code, down } => {
            format!("key-{}:{}", if down { "down" } else { "up" }, format_key(code))
        }
        OutputEvent::MouseButton { button, down } => {
            format!("mouse-{}:{}", if down { "down" } else { "up" }, format_mouse_button(button))
        }
        OutputEvent::MouseMove { dx, dy } => format!("mouse-move:{dx}:{dy}"),
        OutputEvent::MouseWheel { x, y } => format!("mouse-wheel:{x}:{y}"),
    }
}

fn format_key(code: u16) -> String {
    if (key::A..=key::Z).contains(&code) {
        let offset = u8::try_from(code - key::A).unwrap_or(0);
        return char::from(b'A' + offset).to_string();
    }
    if (key::DIGIT_1..=key::DIGIT_9).contains(&code) {
        let offset = u8::try_from(code - key::DIGIT_1).unwrap_or(0);
        return char::from(b'1' + offset).to_string();
    }
    if code == key::DIGIT_0 {
        return "0".to_owned();
    }
    if (key::F1..=key::F12).contains(&code) {
        return format!("F{}", code - key::F1 + 1);
    }

    let name = match code {
        key::ENTER => "Enter",
        key::ESCAPE => "Escape",
        key::BACKSPACE => "Backspace",
        key::TAB => "Tab",
        key::SPACE => "Space",
        key::LEFT_CONTROL => "LeftControl",
        key::LEFT_SHIFT => "LeftShift",
        key::LEFT_ALT => "LeftAlt",
        key::LEFT_META => "LeftMeta",
        key::RIGHT_CONTROL => "RightControl",
        key::RIGHT_SHIFT => "RightShift",
        key::RIGHT_ALT => "RightAlt",
        key::RIGHT_META => "RightMeta",
        key::ARROW_RIGHT => "ArrowRight",
        key::ARROW_LEFT => "ArrowLeft",
        key::ARROW_DOWN => "ArrowDown",
        key::ARROW_UP => "ArrowUp",
        _ => return format!("0x{code:02x}"),
    };
    name.to_owned()
}

fn format_mouse_code(code: u16) -> String {
    MouseButton::try_from(u8::try_from(code).unwrap_or(u8::MAX))
        .map_or_else(|()| code.to_string(), format_mouse_button)
}

fn format_mouse_button(button: MouseButton) -> String {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        MouseButton::Back => "back",
        MouseButton::Forward => "forward",
    }
    .to_owned()
}

fn print_usage() {
    eprintln!(
        "Spellwire native VM utility\n\n\
         Usage:\n\
           spellwire-sim inspect <program.spellwire.bin>\n\
           spellwire-sim simulate <program.spellwire.bin> <event>...\n\n\
         Event syntax:\n\
           key-down:Q\n\
           key-up:LeftShift\n\
           mouse-down:left\n\
           key-down:0x14:synthetic\n\n\
         This utility exercises the compiled native VM. It does not install a global OS input hook."
    );
}

#[cfg(test)]
mod tests {
    use super::{parse_event, parse_key, parse_mouse_button};
    use spellwire_core::{key, Edge, InputDevice, InputSource, MouseButton};

    #[test]
    fn parses_symbolic_keyboard_event() {
        let event = parse_event("key-down:Q").unwrap();
        assert_eq!(event.device, InputDevice::Keyboard);
        assert_eq!(event.code, key::Q);
        assert_eq!(event.edge, Edge::Down);
        assert_eq!(event.source, InputSource::Physical);
    }

    #[test]
    fn parses_synthetic_mouse_event() {
        let event = parse_event("mouse-up:right:synthetic").unwrap();
        assert_eq!(event.device, InputDevice::MouseButton);
        assert_eq!(event.code, MouseButton::Right as u16);
        assert_eq!(event.edge, Edge::Up);
        assert_eq!(event.source, InputSource::Synthetic);
    }

    #[test]
    fn parses_hid_codes_and_aliases() {
        assert_eq!(parse_key("0x14").unwrap(), key::Q);
        assert_eq!(parse_key("Left-Ctrl").unwrap(), key::LEFT_CONTROL);
        assert_eq!(parse_mouse_button("forward").unwrap(), MouseButton::Forward);
    }
}
