# Revisão de Design de Produto, UX e Casos de Uso — Etapix

- **Data:** 2026-11-11
- **Escopo:** Design de produto/UX, consistência visual, casos de uso ponta a ponta, matriz de paridade com o Microsoft Project e auditoria de ajuda/documentação.
- **Método:** Inspeção estática integral de `app/src/{main,ribbon,icons,gantt,rows,dialogs,calendar_window,task_usage,export,fonts,logo,doc,fmt,mpp,snapshot}.rs`, `app/src/help/**`, `app/src/bin/etapix-mcp/**`, `README.md`, `docs/GUIA_TUTORIAIS_ETAPIX.md` e `integrations/**` (sem execução de build/testes, conforme protocolo da etapa).

---

## 1. Revalidação dos pontos existentes do escopo (pontos 4, 5 e 6)

### Ponto 4 — Altura individual por linha (hoje uma altura para todas)
- **Status:** **Continua** | **Gravidade:** média | **Esforço:** M | **Tipo:** certeza (lido no código).
- **Evidência:** `app/src/main.rs:216` (`const ROW: f32 = 20.0`), `app/src/main.rs:419` (`row_h: f32` escalar único em `App`), `app/src/main.rs:1936-1952` (arrastar a borda inferior de qualquer linha na coluna `Col::Num` altera `self.row_h` para todas as linhas), `app/src/gantt.rs:338-346` e `app/src/help/content.rs:112`.
- **O que propõe:** manter `row_h: f32` como altura padrão e acrescentar `row_heights: HashMap<i64, f32>` indexado pelo `uid` da tarefa, expondo em `App` helpers `row_y(shown_row)` e `shown_row_at_y(y)` usados tanto por `grid` quanto por `chart` e `gantt::interact`.

### Ponto 5 — `gantt::interact` e a grade usam `shown`; telas novas devem usar `App::task_at` / `row_of`
- **Status:** **Mudou (atendido na grade/Gantt, mas violado em 3 fluxos adjacentes)** | **Gravidade:** alta | **Esforço:** P | **Tipo:** certeza (lido no código).
- **Evidência:**
  1. `app/src/main.rs:1794, 1869, 1883, 1957, 2014-2025` e `app/src/gantt.rs:337-347` usam `shown`, `task_at` e `row_of`.
  2. **Violação 1 (`Localizar`):** `app/src/dialogs.rs:67, 80-84` busca em `0..self.n()` (incluindo tarefas ocultas por filtro ou resumo recolhido) e calcula `self.scroll_y = (i as f32 * self.row_h - 3.0 * self.row_h).max(0.0)` com o índice bruto `i` em vez de `self.row_of(i)`.
  3. **Violação 2 (`Uso da Tarefa`):** `app/src/task_usage.rs:90-94` percorre `0..doc.project.tasks.len()`, ignorando filtro, ordenação e resumos recolhidos (`rows::Display`).
  4. **Violação 3 (`Exportar PDF do Gantt`):** `app/src/export.rs:331, 363, 541-549` exporta `0..doc.project.tasks.len()` em vez de `shown`.
- **O que propõe:** em `dialogs.rs:80-84`, expandir ancestrais recolhidos da tarefa localizada, limpar o filtro se ela não pertencer a `shown` e rolar usando `self.row_of(i)`; em `task_usage::show` e `export::gantt_pdf`, receber `&[usize]` (`shown`).

### Ponto 6 — Botões da faixa de opções que ainda respondem "Em breve"
- **Status:** **Continua (reduzido: `Alterar Período de Trabalho`, `Uso da Tarefa` e `Exportar PDF` já foram ativados; restam 52 botões `soon`/`soon_big` e 4 checkboxes desabilitados)** | **Gravidade:** média | **Esforço:** M | **Tipo:** certeza (lido em `app/src/main.rs:1085-1104` e `app/src/ribbon.rs:66-479`).
- **Lista completa dos 52 botões `soon`/`soon_big` e 4 checkboxes fixos desabilitados:**
  - **Aba Tarefa (11 botões):** `Pincel de Formatação` (`ribbon.rs:83`), `Negrito` (`ribbon.rs:91-93`), `Itálico` (`ribbon.rs:91-93`), `Sublinhado` (`ribbon.rs:91-93`), `Inativar` (`ribbon.rs:130`), `Inspecionar` (`ribbon.rs:142`), `Modo` (`ribbon.rs:163`), `Entregável` (`ribbon.rs:178`), `Anotações` (`ribbon.rs:187`), `Detalhes` (`ribbon.rs:188`), `Preencher` (`ribbon.rs:206`).
  - **Aba Recurso (7 botões):** `Organizador de Equipe` (`ribbon.rs:213`), `Pool de Recursos` (`ribbon.rs:219`), `Informações` (`ribbon.rs:229`), `Anotações` (`ribbon.rs:231`), `Detalhes` (`ribbon.rs:232`), `Opções de Nivelamento` (`ribbon.rs:246`), `Próxima Superalocação` (`ribbon.rs:250`).
  - **Aba Relatório (5 botões):** `Introdução` (`ribbon.rs:422`), `Personalizado` (`ribbon.rs:423`), `Relatórios Recentes` (`ribbon.rs:424`), `Comparar Projetos` (`ribbon.rs:427`), `Relatórios Visuais` (`ribbon.rs:433`).
  - **Aba Projeto (8 botões):** `Subprojeto` (`ribbon.rs:440`), `Obter Suplementos` (`ribbon.rs:443`), `Campos Personalizados` (`ribbon.rs:449`), `Vínculos Entre Projetos` (`ribbon.rs:450`), `EAP` (`ribbon.rs:452`), `Data de Status: NA` (`ribbon.rs:471`), `Atualizar Projeto` (`ribbon.rs:472`), `Ortografia` (`ribbon.rs:476`).
  - **Aba Exibir (13 botões + 1 checkbox):** `Diagrama de Rede` (`ribbon.rs:265`), `Calendário` (`ribbon.rs:266`), `Organizador de Equipe` (`ribbon.rs:273`), `Uso do Recurso` (`ribbon.rs:274`), `Outros Modos de Exibição` (`ribbon.rs:279`), `Tabelas` (`ribbon.rs:301`), `[Sem Realce]` (`ribbon.rs:304`), `[Sem Grupo]` (`ribbon.rs:313`), `Nova Janela` (`ribbon.rs:358`), `Alternar Janelas` (`ribbon.rs:360`), `Organizar Tudo` (`ribbon.rs:361`), `Ocultar` (`ribbon.rs:362`), `Macros` (`ribbon.rs:366`); checkbox desabilitado `Detalhes` (`ribbon.rs:353`, sem tooltip `SOON`).
  - **Aba Formato (8 botões + 3 checkboxes):** `Estilos de Texto` (`ribbon.rs:372`), `Linhas de Grade` (`ribbon.rs:374`), `Layout` (`ribbon.rs:375`), `Inserir Coluna` (`ribbon.rs:379`), `Configurações de Coluna` (`ribbon.rs:381`), `Campos Personalizados` (`ribbon.rs:382`), `Formatar` (`ribbon.rs:386`), `Desenho` (`ribbon.rs:405`); checkboxes desabilitados `Tarefas Atrasadas` (`ribbon.rs:390`), `Tarefa de Resumo do Projeto` (`ribbon.rs:400`), `Tarefas de Resumo` (`ribbon.rs:401`).
- **O que propõe:** ligar imediatamente os botões cuja lógica já existe ou é trivial (`Formato > Inserir Coluna` abrindo o popup de `Col::Add`; `Formato > Tarefas Atrasadas` destacando barras atrasadas; `Projeto > Data de Status`; `Tarefa > Anotações` e `Recurso > Informações`) e diferenciar visualmente "Em breve" de "Requer seleção" (ver achado G4).

---

## 2. Parte gráfica (Design de Produto e UX)

### G1 — Paleta de cores duplicada e divergente em 7 módulos
- **Gravidade:** baixa | **Esforço:** P | **Tipo:** certeza (`app/src/logo.rs:6-11`, `app/src/main.rs:161-171`, `app/src/icons.rs:5-9`, `app/src/gantt.rs:12-20`, `app/src/dialogs.rs:9`, `app/src/calendar_window.rs:14-16`, `app/src/help/mod.rs:17-21`, `app/src/export.rs:24-38`).
- **Diagnóstico:** as mesmas cores institucionais são redeclaradas com nomes distintos (`GREEN` é `#0B6B66` em `main.rs:161`, mas `#14C38E` em `icons.rs:7`, onde `#0B6B66` se chama `BLUE`). Tons neutros e de erro variam sem motivo (`ERROR_RED` `rgb(190,40,40)` em `calendar_window.rs:15` vs. `RED` `rgb(196,62,54)` em `icons.rs:9` vs. `rgb(180,65,50)` em `gantt.rs:424`).
- **O que o Project faz e o que copiar:** centralizar tokens semânticos num módulo `theme.rs` (`BRAND_TEAL`, `ACCENT_EMERALD`, `TEXT_NAVY`, `TEXT_MUTED`, `SURFACE_RIBBON`, `SURFACE_HEADER`, `DANGER_RED`, `CRITICAL_CORAL`), preparando também terreno para tema escuro.

### G2 — Escolha de tamanho de fonte (`Tarefa > Fonte`) só afeta células da grade
- **Gravidade:** média | **Esforço:** P | **Tipo:** certeza (`app/src/main.rs:1278-1284, 1754, 1813`, `app/src/gantt.rs:65, 153`, `app/src/task_usage.rs:80-112`).
- **Diagnóstico:** quando o usuário aumenta `text_size` (até 24 pt), apenas o texto interno das células da grade cresce. O cabeçalho da grade trava em `self.text_size.min(12.0)` (`main.rs:1754`), os nomes de recursos ao lado das barras do Gantt continuam em `11.0` (`gantt.rs:153`), e `Uso da Tarefa` ignora `text_size`.
- **O que o Project faz e o que copiar:** no Project, o tamanho da fonte de planilha/Gantt escala os rótulos das barras e o cabeçalho da tabela proporcionalmente. Aplicar `text_size` (com piso/teto razoáveis) aos rótulos do Gantt, ao cabeçalho da grade e ao `task_usage`.

### G3 — Ícones repetidos, sem estado ativo e botões sem dica de ferramenta (tooltip)
- **Gravidade:** média | **Esforço:** P | **Tipo:** certeza (`app/src/icons.rs:341-344, 391-412, 450-452`, `app/src/ribbon.rs:101-121, 236-244`, `app/src/main.rs:1162-1163`).
- **Diagnóstico:**
  1. `small_button_response` (`main.rs:1162-1163`) só exibe tooltip quando `compact == true` ou `!enabled`. Por isso, os 9 botões sempre sem texto da aba `Tarefa` (`0%`, `25%`, `50%`, `75%`, `100%`, `Avançar`, `Recuar`, `Vincular`, `Desvincular` em `ribbon.rs:101-120`) e os botões de modo na barra de status (`main.rs:2129, 2135`) **não têm nenhum tooltip quando habilitados**.
  2. `Icon::WorkTime` e `Icon::Month` são aliases de `Icon::Calendar`; `Icon::Links` é alias de `Icon::Link`; `Nivelar Seleção`, `Nivelar Recurso` e `Nivelar Tudo` usam o mesmo `Icon::Level`.
  3. Não há estado visual "pressionado/ativo" (`selected`) nos botões de modo de exibição (`Gráfico de Gantt`, `Uso da Tarefa`, `Planilha de Tarefas`) na faixa nem na barra de status.
- **O que o Project faz e o que copiar:** todo botão de ícone no Project tem tooltip com nome + atalho (ex.: *"Vincular as Tarefas Selecionadas (Ctrl+F2)"*) e fundo destacado quando o modo está ativo. Passar dica explícita para botões sem rótulo e desenhar fundo `SELECTED` no modo ativo.

### G4 — Indistinção visual entre comando "Em breve" (`SOON`) e comando que só exige seleção (`NOT_YET`)
- **Gravidade:** média | **Esforço:** P | **Tipo:** certeza (`app/src/main.rs:1081-1121, 1158-1163`).
- **Diagnóstico:** `soon`/`soon_big` e botões reais desabilitados (`enabled = false` por falta de tarefa selecionada) usam a mesma cor `Color32::from_gray(160)` e o mesmo véu `FADE`. Um usuário novo vê metade da faixa cinza e não sabe o que passa a funcionar ao clicar numa tarefa.
- **O que o Project faz e o que copiar:** manter os comandos implementados mas sem seleção apenas levemente esmaecidos e, nos comandos `SOON`, desenhar um pequeno selo discreto ou tracejado/opacidade menor (ou permitir ocultar comandos não implementados via preferência).

### G5 — Hierarquia da faixa de opções: aba contextual `Formato` fora de lugar e colapso abrupto
- **Gravidade:** baixa | **Esforço:** P | **Tipo:** certeza (`app/src/main.rs:288-298, 1234-1249`, `app/src/ribbon.rs:23-25, 46-61`).
- **Diagnóstico:** `Formato` aparece depois de `Ajuda` sem destaque de aba contextual ("Ferramentas de Gráfico de Gantt") e continua visível mesmo quando `View::Usage` está ativo (onde nenhuma opção de `Formato` tem efeito). Além disso, o grupo `Ajuda > Como usar` é repetido em todas as abas (`ribbon.rs:46-53`).
- **O que o Project faz e o que copiar:** posicionar `Formato` antes de `Ajuda` com uma faixa superior esmeralda (`EMERALD`) identificando-a como contextual ao Gantt/Planilha, e mover o botão `Como usar` para o canto direito fixo da barra de abas (ao lado do título do arquivo), liberando largura na faixa de opções.

### G6 — Legibilidade do Gantt: linha de hoje da mesma cor das barras, sem linhas horizontais de grade e setas II/TT/IT desenhadas como TI
- **Gravidade:** alta | **Esforço:** M | **Tipo:** certeza (`app/src/gantt.rs:12, 20, 68-103, 109-124, 217-258`, `app/src/main.rs:1993-2039`).
- **Diagnóstico:**
  1. **Vínculos não-TI desenhados na âncora errada:** `gantt::links` (`gantt.rs:222-226`) lê sempre `xs = start` da sucessora e `pf = finish` da predecessora, ignorando `link.kind` (`StartToStart`, `FinishToFinish`, `StartToFinish`). Uma relação `II` ou `TT` é desenhada visualmente como se fosse `TI`!
  2. **Escala de tempo incompleta:** `Exibir > Escala de Tempo` oferece `Horas`, `Dias`, `Semanas` e `Meses` (`ribbon.rs:320-327`), mas `gantt::header` (`gantt.rs:68-103`) só tem 2 modos (`s.day >= 10.0` → Semanas/Dias; `< 10.0` → Meses/Semanas). Selecionar `Horas` mostra letras de dias (`D S T Q Q S S`), nunca horas.
  3. **Cor da linha de hoje e falta de linhas de grade:** `TODAY` (`#14C38E`, `gantt.rs:20`) é idêntica a `BAR` (`#14C38E`, `gantt.rs:12`), desaparecendo sobre as barras; e o corpo do Gantt não tem linhas horizontais divisórias de linha (`main.rs:2005-2023`), dificultando parear tarefas e barras em monitores largos.
- **O que o Project faz e o que copiar:** no Project, cada tipo de vínculo (`TI`, `II`, `TT`, `IT`) sai e chega na extremidade correspondente da barra (início ou término); o fundo do Gantt desenha linhas horizontais suaves (`LINE`) alinhadas à grade; a linha de hoje usa cor contrastante (ex.: laranja/vermelho tracejado com rótulo no topo); e a escala de tempo adapta os 2 níveis para Horas (Dia/Hora) e Meses (Ano ou Trimestre/Mês).

### G7 — DPI alto, telas 1366×768 com escala 150% e ausência de tema escuro
- **Gravidade:** média | **Esforço:** M | **Tipo:** certeza para o código (`app/src/main.rs:65, 475, 2322-2339`), estimativa geométrica para 150% DPI.
- **Diagnóstico:** em uma tela 1366×768 com zoom de 150% do Windows (~910×512 pt lógicos), `split: 640.0` deixa apenas ~240 pt para o Gantt, e a soma das faixas superiores fixas (`tabs` 26 + `ribbon` 104 + `help-banner` 28 + `timeline` 64 + `HEAD` 40 = 262 pt) mais `status` (24 pt) consome ~56% da altura útil. O tema claro é fixo em `setup_style` (`Visuals::light()`, `main.rs:65`).
- **O que propõe:** limitar `self.split` inicial a `min(640.0, screen_w * 0.52)`, permitir recolher a faixa de opções (duplo clique na aba ou botão `^`, como no Project) e extrair cores para suportar tema claro/escuro.

### G8 — Acessibilidade: alvos de clique minúsculos (5–12 px), falta de rótulos AccessKit e contraste na Linha do Tempo
- **Gravidade:** média | **Esforço:** M | **Tipo:** certeza (`app/src/main.rs:1110, 1145, 1406, 1843, 1938, 2065`, `app/src/gantt.rs:328, 367`).
- **Diagnóstico:**
  1. Alvos abaixo do mínimo ergonômico: triângulo de resumo `12×12 px` (`main.rs:1843`), borda de altura de linha `6 px` (`main.rs:1938`), divisor grade/Gantt `5 px` (`main.rs:2065`), alça de progresso no Gantt `±3×2,5 px` (`gantt.rs:328`).
  2. Controles customizados (`big_button`, `small_button_response`, abas, QAT) usam `allocate_exact_size` sem `resp.widget_info(...)`, ficando invisíveis a leitores de tela e sem navegação por `Tab`.
  3. Texto de placeholder da Linha do Tempo (`Color32::from_gray(140)` sobre `#EEF3F6` em `main.rs:1406`) tem contraste estimado de ~2,6:1 (abaixo de 4,5:1 WCAG AA).
- **O que propõe:** ampliar áreas de toque (`16×16 px` no triângulo, `8–10 px` nos divisores e alças do Gantt), anexar `WidgetInfo::labeled(WidgetType::Button, true, label)` nos botões pintados à mão e escurecer o texto de placeholder para `MUTED` (`rgb(100,110,120)`, contraste ~4,6:1).

### G9 — Telas vazias mudas e erros confinados à barra de status
- **Gravidade:** média | **Esforço:** P | **Tipo:** certeza (`app/src/main.rs:1788-1866, 2117-2122, 2191`, `app/src/dialogs.rs:258`).
- **Diagnóstico:**
  1. Quando um filtro zera `self.shown` (ex.: `Tarefas Atrasadas` num projeto em dia), a grade fica vazia sem informar que há um filtro ativo.
  2. Erros em janelas modais (`Mover Projeto` em `dialogs.rs:258` e `Informações do Projeto` em `main.rs:2191`) escrevem `self.message` na barra de status no rodapé em vez de dentro da própria janela.
- **O que propõe:** exibir aviso centralizado na grade quando `self.n() > 0 && self.shown.is_empty()` com botão *"Limpar filtro"*, placeholder suave na linha 1 quando `self.n() == 0`, e erro inline nos diálogos (como já faz `calendar_window.rs:424-427`).

### G10 — Aparência e fidelidade do PDF exportado (`export.rs`)
- **Gravidade:** média | **Esforço:** M | **Tipo:** certeza (`app/src/export.rs:186-189, 277-301, 331-396, 485-573`).
- **Diagnóstico:**
  1. `PdfScale::new` (`export.rs:293-301`) espreme todo o projeto nos ~438 pt horizontais de uma única folha A4 (`day_w` cai até `0.4 pt/dia`), tornando ilegíveis cronogramas longos.
  2. Ignora `self.shown` (filtro/ordenação/recolhimento), ignora colunas extras (`TABLE_COLS` fixo em 6 colunas) e ignora `self.gantt_opts` (desenha sempre linha de base e críticas, nunca folga).
  3. `draw_gantt_links` (`export.rs:547-549`) omite vínculos entre tarefas que caem em páginas diferentes.
  4. `estimate_width` (`export.rs:186-189`) usa largura fixa por caractere (`0.51`/`0.55`) para Helvetica proporcional, desalinhando textos à direita.
- **O que propõe:** passar `shown`, `gantt_opts` e colunas visíveis para `gantt_pdf`, usar tabela de larguras reais da Helvetica WinAnsi para `estimate_width` e desenhar indicadores de conector quando um vínculo cruza quebra de página.

---

## 3. Casos de uso (percurso passo a passo no código e pontos de travamento)

### (a) Criar cronograma do zero com EAP, durações e vínculos
- **Percurso no código:** `Doc::blank()` (`doc.rs:53-58`) → digitação em `grid` (`main.rs:1690-1693, 1498-1539`) → `Act::InsertSummary` (`main.rs:974-986`) / `Act::Indent` (`doc.rs:190-198`) → `set_predecessors` (`doc.rs:493-534`) ou arrasto no Gantt (`gantt.rs:420-430`).
- **Onde o usuário trava (certeza):**
  1. **Digitar duração antes do nome numa linha vazia cria tarefa sem nome:** em `commit_edit` (`main.rs:1503`), `let name = if e.col == Col::Name { text.clone() } else { String::new() };`.
  2. **`Insert` / `Inserir > Tarefa` não foca nem limpa `<Nova Tarefa>` ao editar outra coluna:** `Act::InsertTask` (`main.rs:806`) apenas seleciona a linha; se o cursor estava na coluna `Duração`, digitar altera a duração e mantém o nome literal `<Nova Tarefa>`.
  3. **Descompasso de horário inicial no projeto em branco:** `Doc::blank()` cria `start` às `08:00` (`doc.rs:56`), mas o calendário padrão (`Calendar::standard`, `calendar_window.rs:515`) começa às `09:00` (`09:00-12:00, 13:00-18:00`).
  4. **Falta de menu de contexto (botão direito) nas linhas da grade:** no Project, clicar com o botão direito numa linha oferece *Inserir Tarefa, Excluir Tarefa, Recuar, Avançar, Informações*; no Etapix só cabeçalhos de colunas extras têm menu de contexto (`main.rs:1595`).

### (b) Atribuir recursos e nivelar
- **Percurso no código:** `add_resource_window` (`dialogs.rs:134-164`) / `assign_window` (`dialogs.rs:90-132`) / `set_resources` (`doc.rs:339-370`) → `Act::Level` (`doc.rs:548-559`).
- **Onde o usuário trava (certeza):**
  1. **Impossível cadastrar taxas, custos, tipo de recurso ou unidades != 100% pela interface:** `Doc::add_resource` (`doc.rs:373-383`) e `set_resources` (`doc.rs:348-355`) criam sempre `ResourceKind::Work` com `rates: Vec::new()` (R$ 0,00) e `max_units: 1.0`. Não existe planilha editável de recursos (só o relatório estático `Report::Resources` em `dialogs.rs:181-193`).
  2. **Digitar `"Ana[50%]"` na coluna `Nomes dos Recursos` cria um recurso chamado `"Ana[50%]"`:** `set_resources` (`doc.rs:340-358`) divide apenas por `,`/`;` e não interpreta colchetes de unidades como o Project.
  3. **Sem alerta visual de superalocação antes de nivelar:** a coluna `Col::Ind` (`main.rs:1818-1826`) não mostra indicador de recurso superalocado, e `Próxima Superalocação` está em `soon` (`ribbon.rs:250`).
  4. **`Nivelar Seleção`, `Nivelar Recurso` e `Nivelar Tudo` fazem exatamente a mesma coisa:** todos enviam `Act::Level` (`ribbon.rs:236-244`), nivelando o projeto inteiro.

### (c) Salvar linha de base e acompanhar avanço (% concluída, datas reais)
- **Percurso no código:** `Act::Baseline` (`doc.rs:568-574`) → `Act::Percent` / `Col::Percent` (`doc.rs:537-545`) → `Report::InProgress` (`dialogs.rs:207-232`).
- **Onde o usuário trava (certeza):**
  1. **`Definir Linha de Base` não dá feedback visual imediato:** `save_baseline` grava os dados, mas não ativa `self.gantt_opts.baseline` nem exibe confirmação na barra de status; o usuário acha que o botão não funcionou até ir em `Formato > Linha de Base`.
  2. **Sem edição de `Início Real`, `Término Real`, `Duração Real/Restante` ou `Data de Status`:** `set_percent` (`doc.rs:537-545`) assume sempre as datas agendadas (`schedule[i].start`/`finish`) como reais. Se uma tarefa começou 3 dias atrasada e está 50% concluída, o usuário não consegue informar a data real de início sem antes mudar a restrição com a tarefa em 0% (pois `gantt_start` em `doc.rs:268-269` bloqueia mover tarefa com `actual_start`).

### (d) Abrir um `.mpp` vindo do Project, editar e devolver
- **Percurso no código:** `Doc::open` (`doc.rs:60-68`, `app/src/mpp.rs:157-163`) → edições → `Doc::save` (`doc.rs:79-91`, `app/src/mpp.rs:178-191`).
- **Onde o usuário trava (certeza):**
  1. **Caixa `Informações da Tarefa` é somente-leitura:** `main.rs:2148-2169` exibe apenas `ui.label(...)`. O usuário vindo do Project dá `Shift+F2` ou clica em `Informações` esperando editar restrição, tipo de tarefa, prazo final (*deadline*) ou anotações, e encontra uma tela estática.
  2. **Perda silenciosa de restrição específica ao editar `Início` na grade:** `set_start` (`doc.rs:244-254`) sobrescreve qualquer restrição trazida do `.mpp` (`MustStartOn`, `StartNoLaterThan`, `AsLateAsPossible`) por `StartNoEarlierThan`, sem modo na GUI para escolher o tipo de restrição.
  3. **Sem lista de arquivos recentes nem *drag-and-drop*:** o Backstage (`main.rs:1294-1350`) mostra apenas dois cartões fixos (`Projeto em Branco` e `Abrir`), sem histórico de `.mpp` recentes.

### (e) Imprimir/exportar PDF para reunião
- **Percurso no código:** `Act::ExportGantt` (`main.rs:944, 955-970`, `export::gantt_pdf`) e `Act::ExportReports` (`main.rs:945`, `export::all_reports_pdf`).
- **Onde o usuário trava (certeza):**
  1. **Botão `Exportar PDF` na aba `Relatório` não exporta o Gantt:** o usuário vê `Exportar PDF` em `Relatório` (`ribbon.rs:430`), mas ele gera apenas as tabelas de relatórios; a exportação do Gantt fica escondida em `Arquivo > Exportar PDF` (`main.rs:1308`).
  2. **PDF do Gantt ignora o que está na tela:** não respeita filtro, ordenação, resumos recolhidos, colunas adicionadas nem estilos de barra marcados na aba `Formato` (ver G10).

### (f) Calendário com feriados e turno
- **Percurso no código:** `Projeto > Alterar Período de Trabalho` → `calendar_window::CalendarWindow::show` (`calendar_window.rs:299-471`).
- **Onde o usuário trava (certeza):**
  1. **Exceções sem nome:** `Exception` (`pjsched::calendar::Exception`) e `ExceptionDraft` (`calendar_window.rs:46-50`) guardam apenas `from`, `to` e `shifts`, sem campo `name` (ex.: *"Carnaval"*, *"Confraternização Universal"*).
  2. **Botão `Restaurar padrão` (`calendar_window.rs:460-463`) apaga todas as exceções do rascunho de uma vez** sem confirmação, substituindo por `Calendar::standard()`.

### (g) Usar pelo conector MCP (assistente cria e ajusta o cronograma)
- **Percurso no código:** `app/src/bin/etapix-mcp/main.rs:81-321` e `integrations/skill/etapix/SKILL.md`.
- **Onde o usuário trava (certeza):**
  1. **Conflito de arquivo aberto entre GUI e MCP:** o MCP grava direto no disco (`etapix-mcp/main.rs:283`), mas o aplicativo desktop não monitora o timestamp do arquivo aberto (`doc.path`). Se o usuário estiver visualizando o cronograma no Etapix enquanto conversa com o Claude/Codex, não vê as alterações sem reabrir manualmente e, se salvar no app, sobrescreve o trabalho do assistente.
  2. **Operações que a GUI faz mas o MCP não expõe:** o MCP não possui ferramentas para configurar calendário/feriados, cadastrar recursos com taxas/unidades máximas, inserir tarefa em posição específica (só no fim via `adicionar_tarefas`), nem exportar PDF.

### (h) Projeto grande (milhares de linhas): filtro, busca, agrupamento, navegação
- **Percurso no código:** `rows::Display` (`rows.rs:91-154`), `find_window` (`dialogs.rs:53-88`), `move_cursor` e `keyboard` (`main.rs:1655-1719`).
- **Onde o usuário trava (certeza):**
  1. **Sem atalho `Ctrl+F` para `Localizar`:** `keyboard()` (`main.rs:1694-1715`) trata `Ctrl+C/X/V/S/Z/Y/O/N`, mas ignora `Ctrl+F`.
  2. **Navegação por teclado na grade não acompanha a rolagem (`scroll_y` / `grid_x`):** `move_cursor` (`main.rs:1655-1664`) move `self.cursor`, mas nunca atualiza `self.scroll_y` nem `self.grid_x`; apertar `↓` 30 vezes leva o cursor para fora da janela visível. Tampouco existem `PageDown`, `PageUp`, `Home` ou `End`.
  3. **Clique em resultado de `Localizar` rola para a linha errada quando há filtro, ordenação ou resumo recolhido:** usa `i as f32 * self.row_h` (`dialogs.rs:83`) em vez de `self.row_of(i)`.

---

## 4. Matriz de paridade com o Microsoft Project

Legenda: **Tem** (funcional) · **Parcial** (funciona com restrições importantes) · **Em breve** (botão visível mas desabilitado com `SOON`) · **Ausente** (não consta na interface).

### 4.1 Guias e comandos da faixa de opções

| Guia / Grupo do Project | Comando / Recurso no Project | Estado no Etapix | Observação / Evidência |
|---|---|---|---|
| **Arquivo (Backstage)** | Novo, Abrir, Salvar, Salvar como (`.mpp` / `.xml`) | **Tem** | Leitura e escrita nativas de `.mpp` e `.xml` (`main.rs:1308`, `mpp.rs`) |
| **Arquivo** | Imprimir / Exportar PDF (Gantt e Relatórios) | **Parcial** | Exporta PDF A4 paisagem (`export.rs`), mas sem ajuste de escala/página nem filtro |
| **Arquivo** | Arquivos Recentes / Informações / Opções | **Ausente** | Backstage não lista arquivos recentes (`main.rs:1319-1348`) |
| **Tarefa > Exibir** | Gráfico de Gantt (menu de modos) | **Parcial** | Botão simples para Gantt (`ribbon.rs:68`); demais modos ficam na aba `Exibir` |
| **Tarefa > Área de Transf.** | Colar, Recortar, Copiar | **Parcial** | Copia apenas nome, duração e nível (`main.rs:424, 854-857`); perde vínculos e recursos |
| **Tarefa > Área de Transf.** | Pincel de Formatação | **Em breve** | `ribbon.rs:83` |
| **Tarefa > Fonte** | Família e tamanho da fonte | **Parcial** | Escala células da grade (`main.rs:1258-1290`); não aplica por célula |
| **Tarefa > Fonte** | Negrito, Itálico, Sublinhado, Cor, Fundo | **Em breve / Ausente** | N/I/S em `soon` (`ribbon.rs:91-93`); cores de célula ausentes |
| **Tarefa > Cronograma** | 0%, 25%, 50%, 75%, 100%, Marcar no Controle | **Tem** | `ribbon.rs:101-105, 124-126` |
| **Tarefa > Cronograma** | Avançar, Recuar, Vincular, Desvincular | **Tem** | `ribbon.rs:109-120` (faltam tooltips nos 4 botões) |
| **Tarefa > Cronograma** | Respeitar Vínculos / Inativar | **Tem / Em breve** | `Respeitar Vínculos` ativo (`ribbon.rs:127`); `Inativar` em `soon` (`ribbon.rs:130`) |
| **Tarefa > Tarefas** | Agendar Manualmente, Agendamento Automático, Mover | **Tem** | `ribbon.rs:135-162` |
| **Tarefa > Tarefas** | Inspecionar, Modo, Dividir Tarefa, Tarefa Recorrente | **Em breve / Ausente** | `Inspecionar` e `Modo` em `soon` (`ribbon.rs:142, 163`); Dividir/Recorrente ausentes |
| **Tarefa > Inserir** | Tarefa, Resumo, Marco / Entregável | **Tem / Em breve** | Tarefa/Resumo/Marco ativos (`ribbon.rs:168-177`); `Entregável` em `soon` (`178`) |
| **Tarefa > Propriedades** | Informações / Adicionar à Linha do Tempo | **Parcial / Tem** | `Informações` é somente-leitura (`main.rs:2148`); `Linha do Tempo` ativo (`ribbon.rs:189`) |
| **Tarefa > Propriedades** | Anotações, Detalhes | **Em breve** | `ribbon.rs:187-188` |
| **Tarefa > Editar** | Rolar até a Tarefa, Localizar, Limpar / Preencher | **Tem / Em breve** | `ScrollTo`, `Find`, `Delete` ativos (`ribbon.rs:196-205`); `Preencher` em `soon` (`206`) |
| **Recurso > Exibir** | Organizador de Equipe | **Em breve** | `ribbon.rs:213` |
| **Recurso > Atribuições** | Atribuir Recursos / Pool de Recursos | **Parcial / Em breve** | `Atribuir` só marca 100% (`dialogs.rs:115-130`); `Pool` em `soon` (`ribbon.rs:219`) |
| **Recurso > Inserir** | Adicionar Recursos (Trabalho, Material, Custo) | **Parcial** | Só cria recurso de Trabalho a 100% sem taxa (`dialogs.rs:134-164`) |
| **Recurso > Propriedades** | Informações, Anotações, Detalhes | **Em breve** | `ribbon.rs:229-233` |
| **Recurso > Nível** | Nivelar Seleção, Recurso, Tudo, Limpar Nivelamento | **Parcial** | Os 3 botões nivelam o projeto todo (`ribbon.rs:236-249`) |
| **Recurso > Nível** | Opções de Nivelamento, Próxima Superalocação | **Em breve** | `ribbon.rs:246, 250` |
| **Relatório** | Painéis, Recursos, Custos, Em Andamento, Exportar PDF | **Tem** | Modais tabulares + PDF (`ribbon.rs:411-432`, `dialogs.rs:166-237`, `export.rs`) |
| **Relatório** | Introdução, Personalizado, Recentes, Comparar, Visuais | **Em breve** | `ribbon.rs:422-433` |
| **Projeto > Propriedades** | Informações do Projeto / Alterar Período de Trabalho | **Parcial / Tem** | `Informações` só edita data de início (`main.rs:2174`); Calendário completo (`calendar_window.rs`) |
| **Projeto > Propriedades** | Subprojeto, Suplementos, Campos Pers., Vínculos, EAP | **Em breve** | `ribbon.rs:439-452` |
| **Projeto > Cronograma** | Calcular Projeto, Definir Linha de Base, Mover Projeto | **Tem** | `ribbon.rs:458-468` (linha de base 0, sem botão de limpar) |
| **Projeto > Status/Revisão** | Data de Status, Atualizar Projeto, Ortografia | **Em breve** | `ribbon.rs:471-476` |
| **Exibir > Modos** | Gantt, Uso da Tarefa, Planilha de Tarefas, Planilha de Recursos | **Tem / Parcial** | `Planilha de Recursos` abre modal somente-leitura (`ribbon.rs:258-278`) |
| **Exibir > Modos** | Diagrama de Rede, Calendário, Org. Equipe, Uso do Recurso | **Em breve** | `ribbon.rs:265-266, 273-274, 279` |
| **Exibir > Dados** | Classificar, Estrutura de Tópicos, Filtro | **Tem** | 5 ordenações, 9 níveis, 7 filtros (`ribbon.rs:284-312`, `rows.rs`) |
| **Exibir > Dados** | Tabelas, `[Sem Realce]`, `[Sem Grupo]` | **Em breve** | `ribbon.rs:301, 304, 313` |
| **Exibir > Zoom / Divisão** | Escala de Tempo, Ampliar/Reduzir, Projeto Inteiro, Selecionadas, Linha do Tempo | **Parcial / Tem** | Escala `Horas` não desenha horas (`gantt.rs:68`); `Detalhes` desabilitado (`ribbon.rs:353`) |
| **Exibir > Janela/Macros** | Nova Janela, Alternar, Organizar, Ocultar, Macros | **Em breve** | `ribbon.rs:357-367` |
| **Formato** | Tarefas Críticas, Margem de Atraso, Linha de Base, Nº Estrutura | **Tem** | `ribbon.rs:388-399` |
| **Formato** | Estilos de Texto/Barra, Grade, Layout, Colunas, Desenho | **Em breve** | `ribbon.rs:371-406` (`Inserir Coluna` já existe na grade via `Col::Add`) |

### 4.2 Modos de exibição, campos/colunas, tabelas, filtros e diálogos

| Categoria | Microsoft Project | Etapix | Estado e lacunas principais |
|---|---|---|---|
| **Modos de exibição** | Gantt, Gantt de Controle, Planilha de Tarefas, Uso da Tarefa, Planilha de Recursos, Uso do Recurso, Diagrama de Rede, Calendário, Linha do Tempo, Planejador de Equipe | Gantt (com opção de Linha de Base/Folga/Críticas), Planilha de Tarefas, Uso da Tarefa (somente-leitura, 14 dias por página), Linha do Tempo, Relatório de Recursos | **Parcial:** falta uma **Planilha de Recursos editável** (para taxas, unidades máximas, tipo e calendário) e **Uso do Recurso** (para ver quem está superalocado por dia). |
| **Campos / Colunas** | Dezenas de colunas inseríveis em qualquer posição e reordenáveis | 9 colunas padrão (`Id`, `Ind`, `Modo`, `Nome`, `Duração`, `Início`, `Término`, `Predecessoras`, `Recursos`) + 6 extras (`% Concluída`, `Trabalho`, `Custo`, `Margem de Atraso Total`, `Início/Término da Linha de Base`) | **Parcial:** colunas padrão não podem ser ocultadas nem reordenadas (`main.rs:1543-1603`); faltam colunas `EAP/WBS`, `Início Real`, `Término Real`, `Custo Fixo`, `Tipo de Restrição`, `Prazo Final`. |
| **Tabelas, Filtros e Grupos** | Tabelas (Entrada, Custo, Controle, Trabalho, Resumo), Filtros c/ autofiltro, Agrupar por | 7 filtros (`Todas`, `Críticas`, `Não Concluídas`, `Concluídas`, `Marcos`, `Resumo`, `Atrasadas`) e 5 classificações (`Id`, `Início`, `Término`, `Duração`, `Nome`) | **Parcial:** sem troca de Tabela predefinida (ex.: Tabela de Custo ou de Controle) e sem Agrupamento. |
| **Caixas de diálogo** | Informações da Tarefa (editável em abas), Informações do Recurso, Informações do Projeto (início, data de status, calendário), Alterar Período de Trabalho | `Informações da Tarefa` (somente-leitura), `Informações do Projeto` (só data de início), `Alterar Período de Trabalho` (editável), `Atribuir/Adicionar Recursos` | **Parcial:** tornar `Informações da Tarefa` e `Informações do Projeto` editáveis desbloqueia recursos que o motor `pjsched` já calcula e grava no `.mpp`. |

---

## 5. Ajuda e documentação: divergências entre o prometido e o implementado

### D1 — `docs/GUIA_TUTORIAIS_ETAPIX.md` está defasado em relação ao código atual
- **Gravidade:** média | **Esforço:** P | **Tipo:** certeza (comparação direta entre `docs/GUIA_TUTORIAIS_ETAPIX.md` e `app/src/`).
- **Evidências:**
  1. `docs/GUIA_TUTORIAIS_ETAPIX.md:166-167` diz que *"a coluna Término não é editável"* e *"Adicionar Nova Coluna ainda não adiciona colunas"*, mas ambas já funcionam (`app/src/main.rs:1457, 1529-1532, 1569-1593`).
  2. `docs/GUIA_TUTORIAIS_ETAPIX.md:255-256, 320-321, 349-351, 373` lista como "em cinza / inexistentes": `Exportar`, `Alterar Período de Trabalho`, `Uso da Tarefa`, `Classificar`, `Estrutura de Tópicos`, `Filtro` e exportação de relatórios — todos já implementados e ativos.

### D2 — Painel `Como usar` no app (`app/src/help/content.rs`) omite recursos novos das abas `Projeto` e `Exibir`
- **Gravidade:** baixa | **Esforço:** P | **Tipo:** certeza (`app/src/help/content.rs:237-285`).
- **Evidências:**
  1. O tutorial `"projeto"` (`content.rs:244`) não inclui `Alterar Período de Trabalho` (editor de calendários e feriados) em `capabilities` nem em `procedures`.
  2. O tutorial `"exibir"` (`content.rs:266-271`) não inclui `Uso da Tarefa`, `Classificar`, `Estrutura de Tópicos` nem `Filtro` em `capabilities` (só os menciona rapidamente em `limits` na linha 279).

### D3 — Skill do conector (`integrations/skill/etapix/SKILL.md`) e comentário de `main.rs` contradizem o suporte nativo a `.mpp`
- **Gravidade:** média | **Esforço:** P | **Tipo:** certeza (`integrations/skill/etapix/SKILL.md:29-30, 49-50` e `app/src/main.rs:2-3`).
- **Evidências:** `SKILL.md:29-30` instrui o assistente a só criar `.mpp` *"when Microsoft Project is installed, otherwise `.xml`"* e na linha 49 diz *"`.mpp` files open and save through the Microsoft Project installed on the machine"*, enquanto `etapix-mcp/main.rs:79` e `README.md:12, 73-88` já afirmam e implementam leitura, criação e gravação nativas de `.mpp` sem o Project.

### D4 — O que a interface/ajuda promete hoje e o app não entrega por completo
- **Gravidade:** média | **Esforço:** M | **Tipo:** certeza.
- **Evidências:**
  1. **Escala de Tempo `Horas`** (`ribbon.rs:322`, `content.rs:268`): selecionável na caixa de combinação, mas desenha a mesma escala de Semanas/Dias (`gantt.rs:68`).
  2. **`Nivelar Seleção` e `Nivelar Recurso`** (`ribbon.rs:236-241`): botões distintos que executam `Act::Level` no projeto inteiro.
  3. **`Planilha de Recursos`** (`ribbon.rs:276`, `content.rs:267`): promete o modo de exibição de planilha, mas abre apenas o modal estático `Relatório: Recursos`.

---

## 6. Lista priorizada de 15 melhorias (impacto para o usuário × esforço)

| # | Melhoria proposta | Gravidade / Impacto | Esforço | Arquivos tocados | Resumo da implementação |
|---|---|---|---|---|---|
| **1** | **Tornar `Informações da Tarefa` editável (restrição, data, tipo, custo fixo, prazo final, % concluída)** | Alta | M | `app/src/main.rs`, `app/src/dialogs.rs`, `app/src/doc.rs` | Substituir os `ui.label` de `main.rs:2148-2169` por campos editáveis que gravem no modelo `pjsched::model::Task` já suportado pelo motor. |
| **2** | **Criar `Planilha de Recursos` editável (tipo Trabalho/Material/Custo, unidades máx., taxa padrão/hora extra, custo por uso, calendário)** | Alta | M | `app/src/main.rs`, `app/src/ribbon.rs`, `app/src/doc.rs` | Adicionar `View::ResourceSheet` editável (ou tornar o diálogo de recursos editável), permitindo orçar custos e capacidades sem depender de arquivo vindo do Project. |
| **3** | **Corrigir `Localizar` (`Ctrl+F`), `Uso da Tarefa` e `Exportar PDF` para respeitarem `shown` / `row_of`** | Alta | P | `app/src/main.rs`, `app/src/dialogs.rs`, `app/src/task_usage.rs`, `app/src/export.rs` | Ligar `Ctrl+F` em `keyboard()`, usar `self.row_of(i)` (expandindo resumo) em `dialogs.rs:83` e passar `&self.shown` a `task_usage` e `export::gantt_pdf`. |
| **4** | **Manter a célula ativa visível ao navegar com setas e adicionar `PageUp`/`PageDown`/`Home`/`End`** | Alta | P | `app/src/main.rs` | Ao final de `move_cursor` (`main.rs:1655`), ajustar `self.scroll_y` e `self.grid_x` para enquadrar a linha/coluna do cursor; tratar teclas de página em `keyboard()`. |
| **5** | **Desenhar vínculos `II`, `TT` e `IT` nas âncoras corretas do Gantt + linhas horizontais suaves + cor distinta para `Hoje`** | Alta | P | `app/src/gantt.rs`, `app/src/main.rs`, `app/src/export.rs` | Em `gantt::links` (`gantt.rs:219`) e `export.rs:541`, escolher `start`/`finish` conforme `link.kind`; desenhar linhas de grade horizontais no corpo do Gantt e mudar `TODAY` para coral/laranja. |
| **6** | **Copiar e colar tarefas completas (preservando recursos, vínculos internos e duração) e corrigir criação em linha vazia** | Alta | P | `app/src/main.rs`, `app/src/doc.rs` | Guardar struct de tarefa no `clipboard` (`main.rs:424`); em `commit_edit` (`main.rs:1503`), usar nome padrão `<Nova Tarefa>` quando o usuário digitar primeiro a duração numa linha vazia. |
| **7** | **Suportar unidades na atribuição de recursos (`Ana[50%]` na grade e campo de % em `Atribuir Recursos`) + indicador de superalocação** | Alta | M | `app/src/doc.rs`, `app/src/dialogs.rs`, `app/src/main.rs` | Fazer parser de `Nome[X%]` em `doc::set_resources` (`doc.rs:339`), exibir coluna de `%` em `assign_window` e pintar ícone vermelho em `Col::Ind` quando o recurso tiver superalocação. |
| **8** | **Detectar modificação externa do arquivo aberto (integração fluida com o conector MCP) e atualizar `SKILL.md`** | Alta | P | `app/src/main.rs`, `app/src/doc.rs`, `integrations/skill/etapix/SKILL.md` | Guardar `SystemTime` do arquivo em `Doc`; se mudar no disco (ex.: editado pelo Claude/Codex via MCP), oferecer recarregar (ou recarregar se `!dirty`), e corrigir `SKILL.md:29, 49`. |
| **9** | **Ativar comandos da faixa cuja lógica já existe (`Formato > Inserir Coluna`, `Tarefas Atrasadas`, feedback em `Definir Linha de Base`)** | Média | P | `app/src/ribbon.rs`, `app/src/main.rs`, `app/src/gantt.rs` | Ligar `Inserir Coluna` (`ribbon.rs:379`) ao menu de `Col::Add`; ativar `gantt_opts.baseline = true` e aviso ao salvar Linha de Base (`main.rs:827`). |
| **10** | **Completar a escala de tempo do Gantt (`Horas` e `Meses/Anos`)** | Média | P | `app/src/gantt.rs`, `app/src/export.rs` | Adicionar ramo `s.day >= 40.0` (Dia no topo, turnos/horas embaixo) e ramo `s.day < 3.0` (Ano/Trimestre no topo, Mês embaixo) em `gantt::header`. |
| **11** | **Adicionar tooltips em todos os botões de ícone, estado ativo nos seletores de modo e rótulos AccessKit** | Média | P | `app/src/main.rs`, `app/src/ribbon.rs` | Passar tooltip para os 9 botões de `%`/estrutura/vínculo (`ribbon.rs:101-120`), pintar fundo ativo no modo atual (`Gantt`/`Planilha`/`Uso`) e registrar `WidgetInfo::labeled`. |
| **12** | **Menu de contexto (botão direito) nas linhas da grade e nas barras do Gantt** | Média | P | `app/src/main.rs`, `app/src/gantt.rs` | Ao clicar com o botão direito numa linha/barra, abrir menu com *Inserir Tarefa, Inserir Resumo, Excluir, Recuar, Avançar, Atribuir Recursos, Informações*. |
| **13** | **Melhorar o PDF do Gantt (respeitar `gantt_opts` e colunas visíveis, paginação horizontal opcional e largura real de fonte)** | Média | M | `app/src/export.rs`, `app/src/main.rs` | Passar opções de exibição para `export::gantt_pdf`, substituir `estimate_width` por tabela métrica da Helvetica e permitir dividir projetos longos em páginas horizontais. |
| **14** | **Unificar paleta em `theme.rs`, escalar fontes em todas as vistas e ampliar áreas de clique (acessibilidade/DPI)** | Média | M | `app/src/main.rs`, `app/src/gantt.rs`, `app/src/icons.rs`, `app/src/dialogs.rs` | Centralizar constantes de cor, aplicar `self.text_size` no Gantt/`task_usage`, aumentar alvos de toque (triângulo 16 px, divisores 8 px) e permitir recolher a faixa de opções. |
| **15** | **Atualizar `docs/GUIA_TUTORIAIS_ETAPIX.md` e `app/src/help/content.rs` + adicionar nome às exceções de calendário** | Baixa | P | `docs/GUIA_TUTORIAIS_ETAPIX.md`, `app/src/help/content.rs`, `app/src/calendar_window.rs` | Documentar `Alterar Período de Trabalho`, `Uso da Tarefa`, `Filtros/Classificar`, `Col::Add` e `Exportar PDF` nos tutoriais das abas `Projeto` e `Exibir`. |
