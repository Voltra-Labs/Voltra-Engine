# The Voltra MCP server

`voltra-mcp` lets an AI client — Claude Code, Claude Desktop, any MCP client —
work in a Voltra project: read and edit scenes, write assets, run a scene
headless and look at a screenshot of it, build and test. It speaks MCP over
stdio and works on the project's files; no editor needs to be open.

## Running it

```sh
cargo run -p voltra-mcp -- --project . [--assets assets]
```

The asset root is `<project>/assets` unless `--assets` names another. The log
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

## Safety

Local only: stdio to the client that started it. Every path is an asset path
inside the asset root — `..`, absolute paths and links out of the root are
refused. Tools that delete or overwrite say so in their descriptions. cargo is
only ever run with arguments the server builds from checked names.
