# Etapix — revisão da equipe (2026-11-11)

Revisão feita pela equipe de LLMs olhando o Etapix como um time de produto: engenharia e
arquitetura, desempenho, design/UX, casos de uso e QA. Base: `origin/master` em `7660230`.
Nenhum código foi alterado nesta etapa; este documento e o relatório do Gemini são as únicas
entregas.

| Papel | Quem fez | Onde está |
|---|---|---|
| Gerente, QA, engenharia, arquitetura e desempenho | Claude Fable 5.2 | este arquivo |
| Design/UX, casos de uso, paridade com o Project, ajuda | Gemini 4 Argon (Antigravity) | [gemini-design-produto.md](gemini-design-produto.md) |
| Engenharia e desempenho (previsto) | Codex | **não rodou**: a conta atingiu o limite de uso (volta em 10/12/2026). O gerente assumiu a parte dele. |

Duas limitações que afetam o que pôde ser validado:

- **O Microsoft Project não está instalado nesta máquina** (sem `WINPROJ.EXE` nem o ProgID
  `MSProject.Application`). A regressão dos 9 modos contra o Project **não foi rodada de novo**;
  a referência usada foram os arquivos gerados pelo Project em `tests/golden` e `tests/mpp-corpus`.
- Os números de desempenho medem o **motor** (programa de medição fora do repositório, modo
  release, projeto sintético: 1 resumo a cada 10 tarefas, vínculos TI em cadeia, 1 recurso a
  cada 20 tarefas). A taxa de quadros da interface não foi medida; o custo por quadro foi
  deduzido das funções que a interface chama a cada quadro.

## 1. Estado verificado hoje

- `cargo test --workspace`: **99 testes passam, 3 ignorados** (os que precisam do Project ou de
  arquivos extras). `cargo clippy --workspace --all-targets`: limpo.
- Para compilar os testes do app nesta máquina é preciso pôr no ambiente
  `%USERPROFILE%\tools\mingw64\bin` no `PATH` e `...\mingw64\x86_64-w64-mingw32\lib` em
  `LIBRARY_PATH` (falta `libshlwapi.a` na cadeia GNU do rustup). Isso não está no README nem
  nos scripts de teste; só em `tools/build-installer.ps1`.
- Capturas de tela das 12 telas geradas com `ETAPIX_SNAPSHOT_DIR` (preferências apontadas para
  pasta temporária) e conferidas a olho.
- `C:\dev\pjsched-regressao` tem 5 arquivos "modificados" para o git, mas o conteúdo é idêntico
  ao do `master`: não há trabalho perdido ali; a cópia só está com o commit base antigo.

## 2. Revalidação dos pontos que já existiam

| # | Ponto | Situação | Observação |
|---|---|---|---|
| 1 | Calendários de recurso com exceção / meio período (B5-C3..C6, B9-C1, V8) | **Não revalidado no Project** | No motor (XML) esses casos passam nos testes dourados (`tests/golden.rs:130,189`). A diferença conhecida é na gravação `.mpp` conferida pelo Project, que hoje não dá para rodar. |
| 2 | B6-P8: Project zera o % ao receber custo | **Continua** (comportamento do Project) | Caso presente em `tests/golden.rs:152`; nada a corrigir no Etapix. |
| 3 | Progresso: B1-S2, B15 divisão desigual, B17-C5 hora extra, B3-M3 | **Não revalidado no Project** | B17-C5 segue como exceção documentada em `tests/roundtrip.rs:22` (o Project soma hora extra ao reimportar o próprio arquivo). |
| 4 | Altura individual por linha | **Continua** | `row_h` único em `app/src/main.rs:419`; arrastar qualquer borda muda todas. |
| 5 | Telas novas devem usar `task_at`/`row_of` | **Mudou: 3 violações** | Localizar (`dialogs.rs:83`), Uso da Tarefa (`task_usage.rs:91`) e PDF do Gantt (`export.rs`) ignoram filtro, ordenação e resumos recolhidos. |
| 6 | Botões "Em breve" | **Continua: 52 botões e 4 caixas** | Lista completa no relatório do Gemini, seção 1. |
| 7 | `main.rs` com 2352 linhas | **Continua** | Ver seção 4. |
| 8 | Exceção de auditoria do quick-xml | **Continua, vence em 2027-01-31** | Sai ao subir egui/eframe (0.29 hoje) e rfd para versões com zbus 5. |
| 9 | PRs abertos | **Continuam abertos** | #1 e #2 (dependabot, actions v7) com CI **falhando**; #3 (MSI declarativo) com CI verde, bloqueado só pela aprovação. |

## 3. Desempenho (medido)

Tempo em milissegundos, modo release:

| Tarefas | Calendários | CPM | Custos | Acompanhamento | **Recalcular (soma)** | Copiar projeto | **Montar linhas visíveis** | Gravar XML | Ler XML | Nivelar |
|---|---|---|---|---|---|---|---|---|---|---|
| 1.000 | 1,0 | 1,7 | 0,2 | 0,4 | **3,3** | 0,2 | **0,8** | 4,5 | 17,3 | 123 |
| 5.000 | 14,3 | 7,6 | 1,0 | 9,2 | **32,2** | 0,9 | **27,0** | 33,6 | 92,6 | 1.483 |
| 20.000 | 277,1 | 43,8 | 6,9 | 268,0 | **595,8** | 10,0 | **903,8** | 311,1 | 532,2 | não medido |

O CPM em si escala bem (44 ms para 20 mil tarefas). O que cresce com o quadrado é o que está em volta:

| # | Achado | Gravidade | Evidência | Proposta | Esforço |
|---|---|---|---|---|---|
| P1 | **A lista de linhas visíveis é refeita a cada quadro** e custa O(tarefas × resumos): 27 ms com 5 mil tarefas, 904 ms com 20 mil. Com 5 mil tarefas a tela já não chega a 60 quadros por segundo; com 20 mil fica abaixo de 2. | Alta | `refresh_shown` chamado em `central` (`app/src/main.rs:2060`); `rows.rs:113` filtra todas as tarefas para cada pai. Medido. | Montar a lista de filhos uma vez (um `Vec<Vec<usize>>`) e só refazer `shown` quando mudar a versão do documento, o filtro, a ordenação ou o recolhimento. | P |
| P2 | **Cada edição recalcula tudo**, e duas etapas são quadráticas: calendários (277 ms) e acompanhamento (268 ms) com 20 mil tarefas, contra 44 ms do CPM. | Alta | `Calendars::new` percorre todas as atribuições para cada tarefa (`src/calendar.rs:237-242`); `task_work` idem (`src/tracking.rs:32`). Medido. | Indexar atribuições por tarefa uma vez e reutilizar em calendários, custos e acompanhamento. Deve trazer o recálculo de 20 mil tarefas para perto de 60 ms (estimativa). | P |
| P3 | `refresh_summaries` é O(n²): 125 ms com 20 mil tarefas a cada excluir/recuar/avançar. | Média | `app/src/doc.rs:643-648` (`parents.contains` para cada tarefa). Medido. | Marcar os pais num vetor de booleanos. | P |
| P4 | Nivelamento: 1,5 s com 5 mil tarefas, sem aviso de progresso; a interface congela. | Média | `src/leveling.rs:114`; `next_event` varre todas as reservas (`leveling.rs:109`). Medido. | Curto prazo: cursor de espera e mensagem. Depois: reservas ordenadas por recurso. | M |
| P5 | Na grade, cada linha visível realoca a lista de colunas e as colunas Trabalho/Recursos varrem todas as atribuições; `row_of` é busca linear usada a cada quadro. | Média | `self.columns()` dentro do laço de linhas (`main.rs:1801`); `cell_text` `Col::Work` (`main.rs:1442`); `row_of` (`main.rs:662`). Lido, não medido. | Calcular as colunas uma vez por quadro; usar o índice de atribuições de P2; manter um mapa tarefa → linha junto com `shown`. | P |
| P6 | O Gantt gera as formas de **todos** os vínculos a cada quadro, mesmo fora da tela, e calcula a largura total três vezes por quadro varrendo o cronograma inteiro. | Média | `gantt::links` (`gantt.rs:221`); `Scale::width` (`gantt.rs:51`) chamado em `main.rs:2002` e `2104`. Lido, não medido. | Desenhar só vínculos com uma ponta na faixa visível; guardar a data final do projeto junto com o cronograma. | P |
| P7 | Desfazer guarda até 200 cópias inteiras do projeto. Com 20 mil tarefas cada cópia leva 10 ms e a memória pode passar de 1 GB (estimativa, não medido). | Baixa hoje | `UNDO_LIMIT` e `undo: Vec<Project>` em `app/src/doc.rs:41-49`. | Limitar por memória ou guardar só a diferença; não é urgente para os tamanhos usuais. | M |
| P8 | Abrir XML de 24 MB leva 0,5 s e gravar 0,3 s na mesma thread da interface. | Baixa | Medido. Abertura e gravação de `.mpp` grandes não foram medidas. | Aceitável; mostrar cursor de espera. | P |

P1, P2, P3, P5 e P6 são pequenos e independentes do comportamento do motor: os testes dourados
continuam valendo como rede de segurança.

## 4. Engenharia e arquitetura

| # | Achado | Gravidade | Evidência | Proposta | Esforço |
|---|---|---|---|---|---|
| A1 | `main.rs` concentra estado, ações, grade, teclado, janelas e moldura; a faixa e o Gantt já foram separados. | Média | `app/src/main.rs` (2352 linhas; `grid` sozinha tem 255, `run_action` 195). | Extrair `grid.rs` (grade + edição de célula + teclado) e `actions.rs` (`Act` + `run_action`); deixar em `main.rs` só `App`, preferências e `frame_ui`. Fazer **antes** de altura por linha e menu de contexto, que mexem na grade. | M |
| A2 | O conector MCP compila `doc.rs`, `fmt.rs` e `mpp.rs` por `#[path]`, não por uma biblioteca: o mesmo código é compilado duas vezes e nada impede que `doc.rs` passe a depender de código de interface. | Média | `app/src/bin/etapix-mcp/main.rs:9-15`. | Transformar o app em `lib` + dois binários (ou mover `doc` para um crate `etapix-doc`). | M |
| A3 | Projeto em branco começa às 08:00, mas o calendário padrão trabalha das 09:00 às 18:00: a primeira tarefa "perde" uma hora e durações aparecem quebradas. | Média | `Doc::blank` (`doc.rs:56`) × `Calendar::standard` (`src/calendar.rs:48`). Achado do Gemini, conferido. | Iniciar o projeto em branco no primeiro horário útil do calendário. | P |
| A4 | O modelo não guarda anotações, código EAP, linhas de base 1–10, tarefa inativa, tarefa recorrente nem campos personalizados. No `.mpp` isso sobrevive porque a gravação é feita em cima do arquivo original; **no XML, hipótese: é perdido ao salvar** (não testado). | Média | Campos de `Task` em `src/model.rs:51-86`; só `<Baseline><Number>0` em `mspdi_write.rs:344`. | Teste de ida e volta com esses campos; guardar o que não é entendido e devolver intacto. | M |
| A5 | Ambiente de compilação dos testes não documentado (ver seção 1); os dois PRs do dependabot estão vermelhos há 17 dias. | Baixa | Seção 1; PRs #1 e #2. | Script `tools/test.ps1` que monta o ambiente; resolver ou fechar os PRs. | P |
| A6 | Leitor de `.mpp` tem ~230 pontos com `unwrap`/`expect`/fatias em `src/cfb.rs` e `src/mpp.rs`. Não foi auditado um a um: **hipótese** de que um arquivo corrompido derrube o app em vez de mostrar erro. | Média (hipótese) | Contagem por busca; sem teste de arquivo truncado/aleatório. | Teste com arquivos do corpus truncados e bytes trocados; `catch_unwind` na abertura como rede de segurança. | M |

## 5. Parte gráfica e casos de uso

Relatório completo em [gemini-design-produto.md](gemini-design-produto.md). O gerente conferiu no
código os itens abaixo; todos se sustentam.

**Erros visíveis para o usuário (corrigir primeiro)**

- **Vínculos II, TT e IT são desenhados como TI** no Gantt: a seta sempre sai do término da
  predecessora e chega no início da sucessora (`gantt.rs:222-226`). O cálculo está certo; o
  desenho, não.
- **Escala "Horas" não mostra horas**: o cabeçalho só tem dois modos (`gantt.rs:68`).
- **Nivelar Seleção, Nivelar Recurso e Nivelar Tudo fazem a mesma coisa** (`ribbon.rs:236-244`).
- **Localizar rola para a linha errada** com filtro, ordenação ou resumo recolhido (`dialogs.rs:83`).
- **Setas do teclado tiram o cursor da tela**: `move_cursor` não rola a grade; não há PageUp/PageDown/Home/End nem Ctrl+F (`main.rs:1655`).
- **Linha de hoje tem a mesma cor das barras** (`gantt.rs:12` e `20`) e some atrás delas. Na captura de tela o rótulo "Hoje" da linha do tempo ainda fica por cima de "Término".
- **Digitar `Ana[50%]` cria um recurso chamado "Ana[50%]"**; não há como informar unidades, taxa, tipo ou calendário de um recurso pela interface.
- **A skill do conector ainda diz que `.mpp` depende do Project instalado** (`integrations/skill/etapix/SKILL.md:29,49`), o contrário do que o app faz; e o guia de tutoriais lista como inexistentes recursos que já funcionam (`docs/GUIA_TUTORIAIS_ETAPIX.md:166`).

**Lacunas de caso de uso frente ao Project (maior impacto)**

1. **Informações da Tarefa é só leitura**: quem vem do Project espera editar restrição, tipo, prazo final e anotações ali. O motor já calcula tudo isso.
2. **Não existe Planilha de Recursos editável**: sem taxa e sem unidades máximas, custo e nivelamento só fazem sentido em arquivos que vieram do Project.
3. **Acompanhamento incompleto**: não dá para informar início/término real nem data de status; a linha de base é salva sem nenhum retorno visual.
4. **Editar Início troca qualquer restrição por "Não iniciar antes de"**, sem aviso (`doc.rs:244`).
5. **PDF do Gantt ignora a tela** (filtro, colunas, estilos) e espreme o projeto inteiro numa página.
6. **Aplicativo e conector não se enxergam**: se o assistente altera o arquivo aberto, o app não percebe e pode sobrescrever.
7. **Metade da faixa é cinza** e não se distingue "ainda não existe" de "selecione uma tarefa".

**Paridade com o Project** (matriz completa no relatório do Gemini, seção 4): dos modos de
exibição, o Etapix tem Gantt, Planilha de Tarefas, Uso da Tarefa (só leitura) e Linha do Tempo;
faltam Planilha de Recursos, Uso do Recurso, Diagrama de Rede, Calendário e Planejador de
Equipe. Faltam também tabelas predefinidas, agrupamento, colunas ocultáveis/reordenáveis, menu
do botão direito, arquivos recentes, dividir tarefa e tarefa recorrente.

## 6. Plano proposto para a próxima etapa

Ordenado por retorno; cada bloco toca arquivos diferentes para poder ser dividido entre agentes.

| Bloco | Conteúdo | Arquivos | Esforço |
|---|---|---|---|
| **1. Desempenho** | P1, P2, P3, P5, P6 | `app/src/rows.rs`, `src/calendar.rs`, `src/tracking.rs`, `src/cost.rs`, `app/src/doc.rs`, `app/src/gantt.rs` | P |
| **2. Erros visíveis** | Vínculos II/TT/IT, escala Horas e Meses, cor e rótulo de Hoje, linhas horizontais no Gantt, nivelar seleção/recurso de verdade, Localizar + Ctrl+F, cursor sempre visível + teclas de página, hora inicial do projeto em branco | `app/src/gantt.rs`, `app/src/dialogs.rs`, `app/src/main.rs`, `app/src/doc.rs` | P–M |
| **3. Documentação** | Skill do conector, guia de tutoriais, ajuda das guias Projeto e Exibir, receita de compilação dos testes | `integrations/skill/etapix/SKILL.md`, `docs/`, `app/src/help/content.rs`, `README.md` | P |
| **4. Recursos e tarefa editáveis** | Planilha de Recursos editável, `Nome[50%]`, indicador de superalocação, Informações da Tarefa editável, datas reais e data de status | `app/src/doc.rs`, `app/src/dialogs.rs`, nova tela | M–G |
| **5. Estrutura** | A1 (dividir `main.rs`) e A2 (biblioteca compartilhada com o conector) | `app/src/main.rs`, `app/Cargo.toml` | M |
| **6. Acabamento** | Dicas em todos os botões, estado ativo dos modos, diferenciar "Em breve", menu do botão direito, tema central de cores, PDF respeitando a tela, altura por linha | `app/src/main.rs`, `app/src/ribbon.rs`, `app/src/export.rs`, novo `theme.rs` | M |

Os blocos 2, 4 e 6 mexem em `main.rs`; se forem feitos em paralelo, o bloco 5 deve vir antes
ou cada agente trabalha em cópia separada.

**Depende de decisão ou de terceiros**

- Reinstalar o Microsoft Project (ou rodar a regressão em outra máquina) para revalidar os pontos 1 e 3 e conferir os blocos 1 e 2 contra o oráculo.
- PR #3 (MSI) precisa de aprovação; PRs #1 e #2 precisam de correção do CI ou fechamento.
- Codex só volta em 10/12/2026; até lá a divisão é Claude + Gemini.
