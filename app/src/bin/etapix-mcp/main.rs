//! etapix-mcp: a Model Context Protocol server (stdio) that lets an assistant such as Claude or
//! Codex create and edit Etapix projects. Every tool works on a file path: it opens the file
//! (`.xml`, or `.mpp` through the installed Microsoft Project), changes it with the same
//! operations as the app's ribbon, reschedules, and saves it back. The files open in Etapix and
//! in Project.

// Shared with the app; the server uses only part of each.
#[allow(dead_code)]
#[path = "../../doc.rs"]
mod doc;
#[allow(dead_code)]
#[path = "../../fmt.rs"]
mod fmt;
#[path = "../../mpp.rs"]
mod mpp;
mod integrations;

use std::io::{BufRead, Write};
use std::path::Path;

use chrono::NaiveDate;
use doc::Doc;
use serde_json::{json, Value};

fn main() {
    // `instalar [extensão.mcpb]` / `remover`: register in or remove from the assistants.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(cmd) = args.first().filter(|c| *c == "instalar" || *c == "remover") {
        let mcpb = args.get(1).map(std::path::PathBuf::from);
        if let Err(e) = integrations::run(cmd == "instalar", mcpb.as_deref()) {
            eprintln!("Erro: {e}");
            std::process::exit(1);
        }
        return;
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        if let Some(reply) = handle(&msg) {
            let _ = writeln!(out, "{reply}");
            let _ = out.flush();
        }
    }
}

/// Answers one JSON-RPC message; notifications get no answer.
fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg["method"].as_str().unwrap_or("");
    let result = match method {
        "initialize" => json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "etapix", "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            let args = &msg["params"]["arguments"];
            match call(name, args) {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                Err(e) => json!({ "content": [{ "type": "text", "text": format!("Erro: {e}") }], "isError": true }),
            }
        }
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("método desconhecido: {method}") } })),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

const INSTRUCTIONS: &str = "Etapix: cronogramas compatíveis com o Microsoft Project. Use caminhos absolutos. \
Crie com criar_projeto, adicione tarefas com adicionar_tarefas (nivel 1 = topo; uma tarefa seguida de outras com nivel maior vira resumo), \
confira datas e caminho crítico com ler_projeto. Arquivos .mpp exigem o Microsoft Project instalado; .xml não.";

fn tools() -> Value {
    let path = json!({ "type": "string", "description": "Caminho absoluto do arquivo do projeto (.xml ou .mpp)." });
    json!([
        {
            "name": "criar_projeto",
            "description": "Cria um projeto vazio e o salva. .xml (Project XML) funciona em qualquer máquina; .mpp usa o Microsoft Project instalado.",
            "inputSchema": { "type": "object", "required": ["caminho"], "properties": {
                "caminho": path,
                "inicio": { "type": "string", "description": "Data de início: dd/mm/aaaa ou aaaa-mm-dd. Padrão: próxima segunda-feira." }
            }}
        },
        {
            "name": "ler_projeto",
            "description": "Lê o projeto e devolve, em JSON, início e término do projeto e cada tarefa com id, nome, nível, duração, início, término, predecessoras, recursos, % concluída, folga total e se é crítica.",
            "inputSchema": { "type": "object", "required": ["caminho"], "properties": { "caminho": path } }
        },
        {
            "name": "adicionar_tarefas",
            "description": "Acrescenta tarefas ao fim do projeto, na ordem dada, e reagenda. Devolve os ids criados.",
            "inputSchema": { "type": "object", "required": ["caminho", "tarefas"], "properties": {
                "caminho": path,
                "tarefas": { "type": "array", "items": { "type": "object", "required": ["nome"], "properties": {
                    "nome": { "type": "string" },
                    "duracao_dias": { "type": "number", "description": "Dias úteis. 0 = marco. Padrão 1." },
                    "nivel": { "type": "integer", "minimum": 1, "description": "Nível na estrutura (1 = topo). A tarefa acima com nível menor vira resumo." },
                    "predecessoras": { "type": "string", "description": "Ids das predecessoras (término-início), separados por ';', ex. \"2;3\"." },
                    "recursos": { "type": "string", "description": "Nomes separados por vírgula, ex. \"Ana, Bruno\". Recursos novos são criados." },
                    "inicio": { "type": "string", "description": "Não iniciar antes de (dd/mm/aaaa)." }
                }}}
            }}
        },
        {
            "name": "editar_tarefa",
            "description": "Altera campos de uma tarefa pelo id e reagenda. Só os campos informados mudam.",
            "inputSchema": { "type": "object", "required": ["caminho", "id"], "properties": {
                "caminho": path,
                "id": { "type": "integer" },
                "nome": { "type": "string" },
                "duracao_dias": { "type": "number" },
                "inicio": { "type": "string", "description": "Não iniciar antes de (dd/mm/aaaa)." },
                "predecessoras": { "type": "string", "description": "Substitui as predecessoras; \"\" remove todas." },
                "recursos": { "type": "string", "description": "Substitui os recursos; \"\" remove todos." },
                "percentual_concluido": { "type": "integer", "minimum": 0, "maximum": 100 },
                "manual": { "type": "boolean", "description": "true = agendada manualmente; false = automaticamente." },
                "recuar": { "type": "boolean", "description": "true recua (vira subtarefa da de cima); false avança um nível." }
            }}
        },
        {
            "name": "excluir_tarefa",
            "description": "Exclui uma tarefa pelo id (os vínculos com ela também saem).",
            "inputSchema": { "type": "object", "required": ["caminho", "id"], "properties": { "caminho": path, "id": { "type": "integer" } } }
        },
        {
            "name": "nivelar_recursos",
            "description": "Nivela os recursos superalocados (padrões do Project) ou, com limpar=true, desfaz o nivelamento.",
            "inputSchema": { "type": "object", "required": ["caminho"], "properties": { "caminho": path, "limpar": { "type": "boolean" } } }
        },
        {
            "name": "definir_linha_de_base",
            "description": "Salva a linha de base com as datas, trabalho e custos atuais.",
            "inputSchema": { "type": "object", "required": ["caminho"], "properties": { "caminho": path } }
        },
        {
            "name": "salvar_como",
            "description": "Salva uma cópia em outro caminho ou formato (.mpp pelo Microsoft Project instalado, ou .xml).",
            "inputSchema": { "type": "object", "required": ["caminho", "destino"], "properties": { "caminho": path, "destino": path } }
        }
    ])
}

fn text(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn date(s: &str) -> Result<NaiveDate, String> {
    fmt::parse_date(s)
        .or_else(|| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
        .ok_or_else(|| format!("data inválida: {s} (use dd/mm/aaaa)"))
}

fn open(args: &Value) -> Result<(Doc, String), String> {
    let path = text(args, "caminho").ok_or("informe 'caminho'")?;
    Ok((Doc::open(Path::new(&path))?, path))
}

fn row_of(doc: &Doc, id: i64) -> Result<usize, String> {
    doc.project.tasks.iter().position(|t| t.id == id).ok_or_else(|| format!("não existe tarefa {id}"))
}

fn call(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "criar_projeto" => {
            let path = text(args, "caminho").ok_or("informe 'caminho'")?;
            let mut d = Doc::blank();
            if let Some(s) = text(args, "inicio") {
                d.set_project_start(date(&s)?)?;
            }
            d.save(Path::new(&path))?;
            Ok(format!("Projeto criado em {path}, início {}.", fmt::date(d.project.start)))
        }
        "ler_projeto" => {
            let (d, _) = open(args)?;
            Ok(serde_json::to_string_pretty(&summary(&d)).unwrap())
        }
        "adicionar_tarefas" => {
            let (mut d, path) = open(args)?;
            let list = args["tarefas"].as_array().ok_or("informe 'tarefas' (lista)")?;
            let mut ids = Vec::new();
            for t in list {
                let name = t["nome"].as_str().ok_or("cada tarefa precisa de 'nome'")?;
                let days = t["duracao_dias"].as_f64().unwrap_or(1.0);
                let row = d.insert_task(d.project.tasks.len(), name, days)?;
                // insert_task joins the summary above; bring it to level 1, then indent to the target.
                while d.project.tasks[row].parent.is_some() {
                    d.outdent(row)?;
                }
                let level = t["nivel"].as_u64().unwrap_or(1).max(1) as usize;
                while d.outline_level(row) < level {
                    d.indent(row).map_err(|e| format!("'{name}' no nível {level}: {e}"))?;
                }
                if let Some(p) = t["predecessoras"].as_str() {
                    d.set_predecessors(row, p)?;
                }
                if let Some(r) = t["recursos"].as_str() {
                    d.set_resources(row, r)?;
                }
                if let Some(s) = t["inicio"].as_str() {
                    d.set_start(row, date(s)?)?;
                }
                ids.push(d.project.tasks[row].id);
            }
            d.save(Path::new(&path))?;
            Ok(format!("Tarefas criadas: ids {ids:?}. Término do projeto: {}.", finish(&d)))
        }
        "editar_tarefa" => {
            let (mut d, path) = open(args)?;
            let row = row_of(&d, args["id"].as_i64().ok_or("informe 'id'")?)?;
            if let Some(n) = text(args, "nome") {
                d.rename(row, &n);
            }
            if let Some(v) = args["duracao_dias"].as_f64() {
                d.set_duration_days(row, v)?;
            }
            if let Some(s) = text(args, "inicio") {
                d.set_start(row, date(&s)?)?;
            }
            if let Some(p) = text(args, "predecessoras") {
                d.set_predecessors(row, &p)?;
            }
            if let Some(r) = text(args, "recursos") {
                d.set_resources(row, &r)?;
            }
            if let Some(p) = args["percentual_concluido"].as_u64() {
                d.set_percent(row, p as u32)?;
            }
            if let Some(m) = args["manual"].as_bool() {
                d.set_manual(row, m)?;
            }
            match args["recuar"].as_bool() {
                Some(true) => d.indent(row)?,
                Some(false) => d.outdent(row)?,
                None => {}
            }
            d.save(Path::new(&path))?;
            Ok(format!("Tarefa atualizada. Término do projeto: {}.", finish(&d)))
        }
        "excluir_tarefa" => {
            let (mut d, path) = open(args)?;
            let row = row_of(&d, args["id"].as_i64().ok_or("informe 'id'")?)?;
            d.delete_task(row)?;
            d.save(Path::new(&path))?;
            Ok("Tarefa excluída; os ids seguintes foram renumerados.".into())
        }
        "nivelar_recursos" => {
            let (mut d, path) = open(args)?;
            if args["limpar"].as_bool() == Some(true) {
                d.clear_leveling()?;
            } else {
                d.level()?;
            }
            d.save(Path::new(&path))?;
            Ok(format!("Pronto. Término do projeto: {}.", finish(&d)))
        }
        "definir_linha_de_base" => {
            let (mut d, path) = open(args)?;
            d.save_baseline()?;
            d.save(Path::new(&path))?;
            Ok("Linha de base salva.".into())
        }
        "salvar_como" => {
            let (mut d, _) = open(args)?;
            let dest = text(args, "destino").ok_or("informe 'destino'")?;
            d.save(Path::new(&dest))?;
            Ok(format!("Salvo em {dest}."))
        }
        _ => Err(format!("ferramenta desconhecida: {name}")),
    }
}

fn finish(d: &Doc) -> String {
    d.schedule.iter().map(|s| s.finish).max().map(|f| fmt::date(d.dt(f))).unwrap_or_else(|| "-".into())
}

fn summary(d: &Doc) -> Value {
    let tasks: Vec<Value> = (0..d.project.tasks.len())
        .map(|i| {
            let t = &d.project.tasks[i];
            let s = &d.schedule[i];
            json!({
                "id": t.id,
                "nome": t.name,
                "nivel": d.outline_level(i),
                "resumo": t.summary,
                "marco": !t.summary && t.duration == 0,
                "duracao_dias": (d.shown_days(i) * 100.0).round() / 100.0,
                "inicio": d.dt(s.start).format("%d/%m/%Y %H:%M").to_string(),
                "termino": d.dt(s.finish).format("%d/%m/%Y %H:%M").to_string(),
                "predecessoras": d.predecessors_text(i),
                "recursos": d.resources_text(i),
                "percentual_concluido": t.progress.percent_complete,
                "folga_total_dias": (s.total_slack as f64 / doc::DAY as f64 * 100.0).round() / 100.0,
                "critica": s.critical,
                "manual": t.manual.is_some(),
            })
        })
        .collect();
    json!({
        "inicio": d.project.start.format("%d/%m/%Y").to_string(),
        "termino": finish(d),
        "tarefas": tasks,
    })
}
