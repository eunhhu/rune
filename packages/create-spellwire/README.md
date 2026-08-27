# create-spellwire

[한국어](README.ko.md)

Create a Spellwire TypeScript macro project.

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

Generated projects use three commands:

```bash
bun run start  # run once
bun run watch  # run with native hot reload
bun run build  # write the native program and state manifest under dist/
```

`src/main.ts` contains consuming string hotkeys, a native `when` state gate, and the modern overlay in one authoring surface. The compiler extracts realtime handlers into native bytecode while `Spellwire.start()` keeps application code on Bun and owns the unified lifecycle. No manual render/update loop is required.

Skip dependency installation when scaffolding offline:

```bash
bun create spellwire my-automation --no-install
```
