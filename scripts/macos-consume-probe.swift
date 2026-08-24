import CoreGraphics
import Foundation

private let targetKeyCode: CGKeyCode = 40 // ANSI K
private var forwardedTransitions = 0

private func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

let mask = (CGEventMask(1) << CGEventType.keyDown.rawValue)
    | (CGEventMask(1) << CGEventType.keyUp.rawValue)

guard let tap = CGEvent.tapCreate(
    tap: .cgSessionEventTap,
    place: .tailAppendEventTap,
    options: .listenOnly,
    eventsOfInterest: mask,
    callback: { _, type, event, _ in
        if (type == .keyDown || type == .keyUp)
            && event.getIntegerValueField(.keyboardEventKeycode) == Int64(targetKeyCode)
        {
            forwardedTransitions += 1
        }
        return Unmanaged.passUnretained(event)
    },
    userInfo: nil
) else {
    fail("CoreGraphics could not create the tail event tap")
}

guard let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) else {
    fail("CoreFoundation could not create the probe run-loop source")
}

CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
CGEvent.tapEnable(tap: tap, enable: true)

DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
    guard let eventSource = CGEventSource(stateID: .hidSystemState),
          let down = CGEvent(keyboardEventSource: eventSource, virtualKey: targetKeyCode, keyDown: true),
          let up = CGEvent(keyboardEventSource: eventSource, virtualKey: targetKeyCode, keyDown: false)
    else {
        fail("CoreGraphics could not create probe keyboard events")
    }
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
}

DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) {
    CFRunLoopStop(CFRunLoopGetMain())
}

CFRunLoopRun()
print("{\"forwardedTransitions\":\(forwardedTransitions)}")
