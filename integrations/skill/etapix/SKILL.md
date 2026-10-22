---
name: etapix
description: Create, edit and analyse project schedules (cronogramas) in Etapix or Microsoft Project format (.xml / .mpp) through the Etapix MCP tools. Use when the user asks for a cronograma, schedule, Gantt, project plan, EAP/WBS with dates, critical path (caminho crítico), resource leveling (nivelamento), baseline (linha de base), or to read or change a .mpp or Project XML file.
---

# Etapix: cronogramas

Etapix is the team's desktop planner, compatible with Microsoft Project. The `etapix` MCP server
schedules projects with the same engine and saves files that open in Etapix and in Project.

## Tools (MCP server `etapix`)

| Tool | Use |
|---|---|
| `criar_projeto` | New project file. `inicio` = start date (dd/mm/aaaa). |
| `adicionar_tarefas` | Append tasks in order: `nome`, `duracao_dias`, `nivel`, `predecessoras`, `recursos`, `inicio`. |
| `ler_projeto` | Dates, durations, slack, critical flag and structure as JSON. Read before answering about dates. |
| `editar_tarefa` | Change one task by `id` (name, duration, start, links, resources, % complete, manual, indent). |
| `excluir_tarefa` | Delete by `id`; later ids shift down by one. |
| `nivelar_recursos` | Level overallocated resources (`limpar: true` undoes it). |
| `definir_linha_de_base` | Save the baseline. |
| `salvar_como` | Copy to another path or format (.mpp or .xml). |

If these tools are not available, tell the user to install the Etapix connector (the
`Etapix-Claude-Codex` package from the Etapix GitHub releases) instead of writing XML by hand.

## How to build a schedule

1. Ask only for what you cannot infer: start date, and where to save. Default to a `.mpp` next to
   the user's other files when Microsoft Project is installed, otherwise `.xml`. Always use
   absolute paths.
2. `criar_projeto`, then one `adicionar_tarefas` call with the whole structure:
   - `nivel` builds the outline: a task followed by tasks with a higher `nivel` becomes their
     summary. Summaries get their dates from their subtasks: give them no duration, links or
     resources.
   - `duracao_dias` counts working days (Mon–Fri on the standard calendar). `0` makes a milestone
     (marco).
   - `predecessoras` uses the ids the tasks will get: ids are the 1-based row numbers, counting
     summaries, in the order you send them (plus the tasks already in the file). Links are
     finish-to-start.
   - `recursos`: names separated by commas; new names become new resources at 100%.
3. `ler_projeto` and report the finish date, the critical path (tasks with `critica: true`) and
   anything odd, such as a resource on two tasks at once (offer `nivelar_recursos`).

## Editing an existing file

`ler_projeto` first, to get ids and current dates. Make the changes, then read again and tell the
user what moved (finish date, critical path). `.mpp` files open and save through the Microsoft
Project installed on the machine; if that fails, say so and offer `.xml`.

## Conventions

- Write in the user's language; Etapix and the team use Brazilian Portuguese.
- Dates to the user as dd/mm/aaaa; durations as "3 dias".
- A typed start date (`inicio`) is a "start no earlier than" constraint, as in Project.
- Do not invent resources, costs or dates the user did not give; ask, or leave them out.
