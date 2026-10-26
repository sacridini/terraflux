---
title: AI assistants
short: AI assistants (MCP)
description: Connect an AI assistant such as Claude to Terraflux, and build, check and run models by asking in plain words while you watch them appear in the application.
order: 35
---

An AI assistant, such as Claude, Gemini or Codex, can build a Terraflux model for you. You ask in plain words ("read my land use
map of 2010, find the forest, and measure how fragmented it is with MSPA"), and the assistant adds the nodes,
connects them, sets them, checks the model and runs it. If the model is open in the application, you watch every
node appear on the canvas as it is added, and you can change or undo anything the assistant did.

The connection uses the **Model Context Protocol** (MCP), an open standard that lets an assistant use the tools of
another program. `terraflux mcp` is a small server that gives the assistant Terraflux's tools. It runs on your
computer: your maps stay where they are, and the assistant reads only what the tools return (the model, the
problems found in it, and summaries of the results).

## What the assistant can do

| Tool | What it does |
|---|---|
| `list_templates`, `new_model` | Lists the templates (small models that run as they are) and starts a model, empty or from a template |
| `list_operators`, `describe_operator` | Finds operators by what they do; gives an operator's inputs, outputs and settings, with an example node |
| `add_node`, `edit_node`, `remove_node` | Adds a node connected and set, changes it (its connections, settings, note, name), removes it |
| `set_input`, `set_output`, `edit_container` | The model's inputs and outputs, and what a loop or group carries, collects or gives |
| `show_model`, `check_model`, `arrange` | Shows the model as text, checks it and its data, and puts its nodes in tidy columns |
| `run_model`, `inspect_file` | Runs the model, then summarises a result: a raster's classes or statistics, a table's first lines |
| `open_in_app` | Opens the model in the application you have open (a new window when none is, or when yours has edits not saved) |

Every edit is checked the way the application checks the edits you make yourself. A connection of the wrong
type (a table into an input that takes a map), a setting the operator does not have, or a loop of nodes reading
each other is refused, the reason is given to the assistant, and the model file is left as it was. After each
edit the assistant is told the problems the model still has, so it can go on fixing them.

## Connect an assistant

The assistant starts `terraflux mcp` by itself whenever it needs it: you only tell it once where the program is.

=== "Claude Desktop"
    Open **Settings**, then **Developer**, then **Edit Config**. This opens the file
    `claude_desktop_config.json`. Add Terraflux to its `mcpServers` (keep any other servers already there):

    ```json
    {
      "mcpServers": {
        "terraflux": {
          "command": "C:\\Users\\you\\AppData\\Local\\Programs\\Terraflux\\terraflux.exe",
          "args": ["mcp"]
        }
      }
    }
    ```

    The `command` is where Terraflux is installed: on Windows,
    `C:\\Users\\you\\AppData\\Local\\Programs\\Terraflux\\terraflux.exe`, with your user's folder in place of `you`
    (`C:\\Program Files\\Terraflux\\terraflux.exe` from an installer before 1.18; the backslashes doubled, as JSON
    wants them); on macOS,
    `/Applications/Terraflux.app/Contents/MacOS/terraflux`; on Linux, `/usr/bin/terraflux` from the `.deb`
    package or `~/.local/bin/terraflux` from the archive. Restart Claude Desktop. Terraflux's tools appear in the
    tools menu of the message box.

=== "Claude Code"
    In a terminal:

    ```bash
    claude mcp add --scope user terraflux -- terraflux mcp
    ```

    `--scope user` makes Terraflux available in every folder you work in; leave it out to add it to the current
    project only. `claude mcp list` shows the servers added.

=== "Gemini CLI"
    In a terminal:

    ```bash
    gemini mcp add -s user terraflux terraflux mcp
    ```

    `-s user` makes Terraflux available in every folder; without it, it is added to the current project only
    (its `.gemini/settings.json`). The same entry can be written by hand in `~/.gemini/settings.json`:

    ```json
    { "mcpServers": { "terraflux": { "command": "terraflux", "args": ["mcp"] } } }
    ```

=== "Codex"
    In a terminal:

    ```bash
    codex mcp add terraflux -- terraflux mcp
    ```

    This writes the server to `~/.codex/config.toml`, which the Codex command line and its editor extension
    share. By hand, the same entry is:

    ```toml
    [mcp_servers.terraflux]
    command = "terraflux"
    args = ["mcp"]
    ```

=== "Other assistants"
    Any client of the Model Context Protocol that starts local servers can use Terraflux. Give it the command
    `terraflux` (or the full path to the program) with the argument `mcp`; the server speaks over the standard
    input and output. Chat applications in a web browser usually connect only to servers on the internet, and
    cannot start one on your computer.

If the assistant says it cannot find `terraflux`, give the full path to the program instead (see the
Claude Desktop tab for where it is installed).

## Work with it

Start by opening the application, or ask the assistant to open the model there (`open_in_app`): the model opens in
the Terraflux you have open, in place of its own, and the window comes to the front. A new window opens only when
none is open, or when the model in yours has edits not saved (they are never lost). Then ask for what you want, as
you would ask a colleague who knows Terraflux:

> Make a model in `D:\forest\fragmentation.toml` that reads `D:\forest\landcover_2020.tif`, where class 3 is
> forest, and computes MSPA with an edge width of 2. Open it in Terraflux.

> Add the forest's area density at three scales, and write the MSPA map to a GeoTIFF.

> Run it and tell me how much of the forest is core.

A good way to work:

- **Give full paths** to your files and to the model. The assistant may not know which folder you mean.
- **Name your classes.** Say what each value of your maps means ("1 is pasture, 2 is forest"). The assistant
  cannot guess it from the file.
- **Start from a template** when one is close to what you want. Ask "which templates are there for
  connectivity?" and then "start from the habitat network template, with my map in place of the drawn one".
- **Look at the canvas.** The application follows the model's file: every edit appears there as it is made,
  and the status bar says so. Click the nodes to see their outputs, as with any model.
- **Read the notes.** Ask the assistant to write in each node's note why it is there. The notes stay in the
  model and are shown in the node's panel.

!!! tip "Undo what the assistant did"
    An edit made by the assistant is an edit like yours: ++ctrl+z++ in the application undoes it, and
    ++ctrl+shift+z++ makes it again. Save the model afterwards to keep the version you want.

!!! warning "Your edits and the assistant's at the same time"
    If you change the model in the application and have not saved it when the assistant changes the file, the
    application asks which to keep: **Read it** takes the assistant's version (++ctrl+z++ brings yours back),
    **Keep mine** keeps yours (saving replaces the file). Nodes you only moved are not a conflict: the
    assistant's version is taken with your nodes where you put them. To avoid the question, save before asking
    the assistant for more.

## Check what the assistant built

An assistant can be wrong: it may choose an operator that does not fit your question, or a setting that does not
suit your landscape. The checks catch a model that cannot run, not a model that answers the wrong question.
Before you use the results:

- read the model on the canvas, node by node, and make sure each step is one you would have chosen;
- check the settings that carry ecological meaning (an edge width, a dispersal distance, a threshold) against
  your species and your data;
- compare a result with what you know of the place, as you would for any model you built yourself.

Terraflux keeps the model as a plain file, so the work stays reproducible: what the assistant built runs again
with `terraflux run`, without the assistant.
