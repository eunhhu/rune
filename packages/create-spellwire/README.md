# create-spellwire

[한국어](README.ko.md)

Create a Spellwire TypeScript macro project.

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

Generated projects expose only three workflows:

```bash
bun run start  # run once
bun run watch  # run with native hot reload
bun run build  # write the native program and state manifest under dist/
```

`src/main.spellwire.ts` contains realtime VM logic. `src/app.ts` contains the generated state-driven modern overlay and unified `Spellwire.start()` lifecycle; no manual render/update loop is required.

Skip dependency installation when scaffolding offline:

```bash
bun create spellwire my-automation --no-install
```
