# RustGames Agent Instructions

These instructions apply to all Rust game projects in this workspace.

## Project Standards

- Build games with Rust, `macroquad`, and the shared `macroquad-toolkit` by default.
- Treat missing runtime, rendering, input, asset, or platform behavior as potential `macroquad-toolkit` upgrades before creating project-local alternatives.
- Only diverge from the shared toolkit when an existing project has a clear, established alternative or the need is genuinely game-specific.
- Keep source files under 800 lines. Split large files by responsibility before they become difficult to scan or test.
- Prefer small modules with explicit ownership of input, update logic, rendering, assets, and game state.
- Use Rust's named module source filenames (`foo.rs`, `foo/bar.rs`) instead of `foo/mod.rs`. Do not create new `mod.rs` files.
- Keep gameplay logic deterministic where practical. Isolate randomness behind small helper functions or state-owned RNG.
- Avoid broad refactors while making focused changes. Match the style, naming, and structure already present in each project.
- Use clear error handling for asset loading, save/load, publishing, and platform integration.
- Do not introduce new dependencies unless they remove real complexity or match an established project pattern.

## Macroquad Conventions

- Use `macroquad` for the runtime loop, input, drawing, textures, audio, and timing.
- Keep drawing code separate from state mutation where possible.
- Treat screen size, scaling, and camera transforms as first-class concerns. Games should remain playable at common desktop browser sizes.
- Avoid hard-coded absolute positions unless they are intentionally tied to a fixed virtual resolution.
- Load assets through project-local asset paths and keep missing asset behavior obvious during publishing.

## Testing And Validation

- Use each project's `publish.ps1` script as the validation path.
- Do not treat running a local instance or local dev server as the required test path unless the user explicitly asks for it.
- After meaningful changes, run `.\publish.ps1` with no parameters from the affected project directory and report whether it passes.
- If `publish.ps1` is missing, blocked, or fails for an unrelated environment reason, report that clearly instead of substituting an unrequested local run.

## File Size Rule

- Keep every `.rs` file below 800 lines.
- Treat a file reaching or approaching 800 lines as a restructure signal, not as a formatting target.
- Do not preserve the limit by stripping useful spacing, compressing formatting, moving a single small function, or making other cosmetic line-count changes.
- If a meaningful change would push a file over the limit, extract a cohesive responsibility into one or more nearby modules before or alongside the change.
- If a touched file is already over 800 lines, make the restructure part of the current task, or queue it as the next work item before considering the task complete.
