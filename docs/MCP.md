# The Voltra MCP server

`voltra-mcp` lets an AI client — Claude Code, Claude Desktop, any MCP client —
work in a Voltra project: read and edit scenes, write assets, run a scene
headless and look at a screenshot of it, build and test. It speaks MCP over
stdio and works on the project's files; no editor needs to be open. With an
editor running, its `editor_*` tools drive that editor live.

## Running it

```sh
cargo run -p voltra-mcp -- --project . [--assets assets] [--editor-port 47810]
```

The asset root is `<project>/assets` unless `--assets` names another;
`--editor-port` is where the `editor_*` tools look for an editor (47810 by
default). The log
goes to stderr; stdout is the protocol's.

To give Claude Code the tools in this repository, add a `.mcp.json` at its
root (build once first — `cargo build -p voltra-mcp` — so the first start is
not a compile):

```json
{
  "mcpServers": {
    "voltra": {
      "command": "cargo",
      "args": ["run", "-q", "-p", "voltra-mcp", "--", "--project", "."]
    }
  }
}
```

## Tools

| Tool | What it does |
| --- | --- |
| `components_list` | Every component a scene entity can carry, whether it names other entities, and its **template** — its default value as RON, the starting point for writing one |
| `assets_list` | A directory of the asset root, with each entry's kind |
| `scenes_list` | Every scene (which is also every prefab) |
| `scene_read` | A scene's entities: id, name, parent, components as RON |
| `scene_create` | A new, empty scene |
| `entity_spawn` | An entity with components (name → RON), a name, a parent |
| `entity_delete` | An entity and everything under it |
| `entity_reparent` | An entity under another, or a root |
| `entity_rename` | An entity's name |
| `component_set` / `component_remove` | One component, by registered name |
| `prefab_instantiate` | A prefab placed in a scene, linked to its file |
| `asset_read` / `asset_write` | A RON asset's text; written only once it reads as what its name says (scene, `.anim.ron`, `.animator.ron`, `.input.ron`, `.atlas.ron`, `.layers.ron`) |
| `run_scene` | The scene run headless for some frames, actions held on the frames given; answers where things ended up (or chosen entities in full), the engine's log, and a screenshot |
| `project_build` / `project_test` / `project_dist` | cargo, with arguments the tool builds |

**Entities** are named by id (the UUID `scene_read` gives) or by a path of
names from a root: `"Player"`, `"Player/Arm"`.

**Components** are RON as scene files hold them. Some components take only the
fields that differ from their defaults; others need every field — start from
`components_list`'s template and change what you need. A value that does not
read is refused with the format's own message.

**Every edit is saved at once, or not at all**: a failed edit leaves the file
byte-identical.

## Driving a running editor

Start the editor with the bridge on:

```sh
cargo run -p voltra-editor -- --bridge          # port 47810
cargo run -p voltra-editor -- --bridge 47900    # another port; pass --editor-port to voltra-mcp
```

The `editor_*` tools then act on what the person has open, and the person
sees each change appear:

| Tool | What it does |
| --- | --- |
| `editor_status` | Scene, play state, selection, entity count, what undo and redo would do |
| `editor_read` | The open scene, live: entities with ids, names, parents, components as RON |
| `editor_spawn` / `editor_delete` / `editor_reparent` / `editor_rename` | As the file tools, on the open scene |
| `editor_component_set` / `editor_component_remove` | One component, by registered name |
| `editor_select` | Select entities (the last the primary); an empty list clears |
| `editor_history` | `undo` or `redo` — the person's edits or an AI's |
| `editor_save` | The scene to its file |
| `editor_play` | `play`, `pause`, `step` (with `frames`), `stop` (the authored scene back) |
| `editor_input` | Hold an input action for some frames while playing |
| `editor_screenshot` | The viewport as last drawn — scene, lights, grade — as an image |

**Every edit is one undo entry**, labelled `AI: spawn`, `AI: set component`
and so on: Ctrl+Z in the editor takes it back like the person's own. Edits
and saves are refused while playing; reads show the live world.

The file tools and the editor tools are separate on purpose: with an editor
open on a scene, edit it through `editor_*` — a file written underneath it
would be overwritten by the editor's next save. No editor listening is an
answer that says how to start one.

## Safety

Local only: stdio to the client that started it; the editor bridge is off
unless `--bridge` is given and listens on the loopback interface only. Every path is an asset path
inside the asset root — `..`, absolute paths and links out of the root are
refused. Tools that delete or overwrite say so in their descriptions. cargo is
only ever run with arguments the server builds from checked names.
