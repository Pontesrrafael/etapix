# Guia de tutoriais do Etapix

Versão: 1.0
Finalidade: orientar a implementação de ajuda contextual, manual e tours guiados no Etapix.
Uso: especificação para quem desenvolve o Etapix (pessoas ou agentes). Segue o formato do
"Guia genérico de tutoriais para aplicações", preenchido com o que o Etapix é e faz hoje.

Implementado no Etapix 0.1.0: Central de Ajuda, painel **Como usar** em cada aba e na grade,
tour guiado da grade e das abas Tarefa, Recurso, Projeto, Exibir, Formato e Relatório, e a
sugestão na primeira visita. O código está em `app/src/help/` (`content.rs` tem todo o texto).
A seção 10 diz o que foi verificado e a seção **Pendências**, o que falta.

## 1. Configuração da aplicação

| Campo | Preenchido |
|---|---|
| Aplicação | Etapix 0.1.0 |
| Objetivo da aplicação | Planejar e acompanhar cronogramas de projeto (tarefas, estrutura, vínculos, recursos, caminho crítico, nivelamento e linha de base) com a mesma interface e os mesmos cálculos do Microsoft Project, lendo e gravando arquivos `.mpp` e Project XML. |
| Público principal | Equipe interna que já usa ou usaria o Microsoft Project: planejadores, coordenadores e gestores de projeto. Parte do público vem do Project e espera os mesmos nomes; parte nunca usou uma ferramenta de cronograma. |
| Módulos prioritários | 1. Grade de tarefas e Gráfico de Gantt. 2. Aba Tarefa. 3. Arquivo (abrir e salvar `.mpp`/`.xml`). 4. Aba Recurso. 5. Aba Projeto. 6. Aba Exibir e Formato. 7. Aba Relatório. 8. Linha do Tempo. 9. Conector para Claude e Codex. |
| Tecnologia e componentes existentes | Rust, interface em egui/eframe 0.29 (modo imediato, sem HTML nem DOM). Faixa de opções em `app/src/ribbon.rs`, janelas em `app/src/dialogs.rs`, grade e Gantt em `app/src/main.rs` e `app/src/gantt.rs`, ícones desenhados em `app/src/icons.rs`. Motor de agendamento `pjsched`. Testes de interface sem janela em `app/src/monkey_tests.rs`. |
| Perfis e permissões | Não há perfis nem login: é um aplicativo de mesa por pessoa, e quem abre um arquivo pode alterá-lo. As únicas restrições são de contexto: comandos que exigem uma tarefa selecionada e Claude Desktop só no Windows (abrir, criar e salvar `.mpp` funcionam em qualquer máquina, sem o Microsoft Project). |
| Identidade visual | Paleta Etapix: verde-petróleo `#0B6B66` (marca, abas, seleção), azul-marinho `#1E2F3E` (texto), esmeralda `#14C38E` (barras, destaques), menta `#4EE7C7` e cinzas claros. Fontes Inter (texto) e Plus Jakarta Sans (títulos), embutidas. Faixa de opções e termos iguais aos do Microsoft Project em português. |
| Progresso entre dispositivos | Local, por usuário do sistema operacional: no mesmo arquivo de configurações que o Etapix já usa (`%APPDATA%\Etapix\settings.txt` no Windows, `~/.config/etapix/settings.txt` no Linux). Não há sincronização. |
| Responsável pela revisão do conteúdo | `[A DEFINIR]`: responsável pelo Etapix na equipe (sugestão: rafael.pontes). |

Indefinido: o responsável formal pela revisão. Todo o resto pode avançar sem essa decisão.

## 2. Objetivo dos tutoriais

Permitir que quem abre o Etapix pela primeira vez monte um cronograma sem treinamento, e que quem
vem do Microsoft Project encontre cada comando onde espera.

As três camadas:

1. **Catálogo de recursos:** uma página **Ajuda > Central de Ajuda** com o que o Etapix oferece e
   por onde começar.
2. **Ajuda contextual:** botão **Como usar** em cada aba da faixa de opções e na grade, com
   procedimentos consultáveis.
3. **Tour guiado:** uma sequência curta que destaca os controles de cada aba e da grade.

Começar por estes pontos, em ordem:

- **Mais frequente:** digitar tarefas na grade, definir duração e predecessoras, recuar e avançar
  (estrutura de tópicos).
- **Mais dúvida:** a sintaxe das predecessoras (`3II`, `2TI+2 dias`), as tarefas manuais e
  automáticas e o nivelamento.
- **Maior impacto de erro:** salvar `.mpp` por cima de um arquivo aberto em outro programa, mover o projeto
  inteiro, nivelar recursos e definir a linha de base (que substitui a anterior).

## 3. Catálogo de recursos

- Abrir pela aba **Ajuda > Central de Ajuda** e por um cartão **Aprender a usar o Etapix** na tela
  **Arquivo > Novo**.
- Agrupar os cartões por finalidade, com estes grupos e cartões:
  - **Montar o cronograma:** Tarefas e duração; Estrutura (resumos e subtarefas); Vínculos e
    predecessoras; Marcos.
  - **Pessoas e custos:** Recursos e atribuições; Nivelamento; Custos.
  - **Acompanhar:** % concluída; Linha de base; Relatórios; Linha do tempo.
  - **Arquivos:** Abrir e salvar `.mpp` e `.xml`; Salvar sem perder alterações.
  - **Exibição:** Zoom e escala de tempo; Colunas e linhas; Fonte; Caminho crítico e folga.
  - **Assistentes:** Etapix no Claude e no Codex.
- Cada cartão mostra ícone (os mesmos de `icons.rs`), título, uma frase com o benefício e as
  ações **Ir para** (abre a aba ou o controle) e **Como usar** (abre a seção do painel).
- Cartões só explicativos, sem tela, são permitidos (por exemplo, **Trabalhar com o Microsoft
  Project** e **Etapix no Claude e no Codex**).
- Sequência inicial sugerida no topo do catálogo:
  1. **Preparar:** Projeto > Informações do Projeto (data de início).
  2. **Primeira operação:** digitar as tarefas, as durações e as predecessoras na grade.
  3. **Acompanhar:** % concluída, linha de base e o relatório Em Andamento.
  4. **Consultar:** Relatório > Painéis e caminho crítico (Formato > Tarefas Críticas).
- Respeitar o contexto: no Linux, o cartão de `.mpp` informa que `.mpp` abre e salva como no Windows, sem o Project, e
  o cartão do Claude Desktop aparece só no Windows.
- Cartões diferentes reutilizam o mesmo tutorial abrindo a seção certa. Por exemplo,
  **Marcos** e **Tarefas e duração** usam o tutorial da aba Tarefa.

Busca no catálogo: opcional na primeira versão (cerca de 20 cartões).

## 4. Ajuda contextual por tela

Botão **Como usar** (ícone de interrogação) no fim de cada aba da faixa de opções, como último
grupo à direita, e no canto do cabeçalho da grade. Continua disponível depois que o tour for
concluído ou dispensado.

### Estrutura do painel

1. Cabeçalho com ícone da aba, título ("Como usar: Tarefa") e botão de fechar.
2. Resumo de uma ou duas frases.
3. Botão **Fazer o tour** ou **Refazer o tour**.
4. **O que você pode fazer aqui.**
5. **Passo a passo**, com procedimentos expansíveis.
6. **Dicas**, incluindo as limitações reais da versão.
7. Link **Central de Ajuda**.
8. Caixa **Sugerir ajuda na primeira vez que abro uma aba**.

### Comportamento

- Painel lateral à direita (`egui::SidePanel::right`), largura padrão de 360 px e redimensionável,
  sem passar de 40% da janela. Em janelas estreitas (menos de 900 px), abre como janela flutuante.
- O primeiro procedimento vem aberto (`egui::CollapsingHeader` com `default_open(true)`) e os
  demais ficam fechados.
- O conteúdo rola; o cabeçalho e o botão de fechar ficam fixos.
- Fecha pelo botão e pela tecla Escape. A tecla Escape não deve ser capturada enquanto uma célula
  da grade estiver em edição, porque ali ela cancela a edição.
- Ao abrir a ajuda de outra aba pelo catálogo, o painel mostra **Ir para a aba e fazer o tour**.
- Usar as cores e fontes do Etapix (constantes de `main.rs`), sem medidas copiadas de outra
  aplicação.

## 5. Conteúdo de cada módulo

Nomes iguais aos da interface do Etapix 0.1.0. Nada aqui promete o que o Etapix não faz:
os comandos em cinza na faixa de opções estão listados em **Limitações**.

### Grade de tarefas e Gráfico de Gantt

**Para que serve:** listar as tarefas do projeto com duração, datas, predecessoras e recursos, e
ver o cronograma em barras ao lado.

**O que você pode fazer aqui:**
- Digitar tarefas novas nas linhas vazias.
- Editar nome, duração, início, predecessoras e nomes dos recursos.
- Ajustar a largura das colunas e a altura das linhas.
- Selecionar várias tarefas (Ctrl e Shift) para os comandos da faixa de opções.

**Procedimento: Criar tarefas**
1. Clique na coluna **Nome da Tarefa** da primeira linha vazia.
2. Digite o nome e pressione Enter. O cursor desce para a linha seguinte.
3. Repita para as próximas tarefas.
4. Clique na coluna **Duração**, digite o número de dias úteis (por exemplo, `3` ou `2,5`) e
   pressione Enter.

**Como conferir:** a tarefa ganha um número na primeira coluna, as datas de **Início** e
**Término** aparecem e uma barra esmeralda surge no Gantt.

**Se não funcionar:** se aparecer "Duração inválida" na barra de status, use só números (`3`,
`2,5`, `3 dias`). Para cancelar uma edição, pressione Escape.

**Procedimento: Vincular tarefas pelas predecessoras**
1. Clique na coluna **Predecessoras** da tarefa que vem depois.
2. Digite o número da tarefa anterior. Para outros tipos de vínculo, acrescente `TI`
   (término-início), `II` (início-início), `TT` (término-término) ou `IT` (início-término), e uma
   defasagem opcional, como `2TI+2 dias` ou `3II-1d`. Separe várias com `;`.
3. Pressione Enter.

**Como conferir:** a data de início se move e uma seta liga as barras no Gantt.

**Se não funcionar:** "predecessora inválida" indica sintaxe fora do padrão acima; "não existe
tarefa N" indica um número que não está na grade; "os vínculos formam um ciclo" indica que uma
tarefa depende, direta ou indiretamente, dela mesma. Nesses casos nada é alterado.

**Procedimento: Ajustar colunas e linhas**
1. Arraste a divisa entre dois títulos de coluna, no cabeçalho. Dois cliques ajustam a coluna ao
   maior texto.
2. Arraste a divisa entre duas linhas, na coluna dos números.

**Como conferir:** o tamanho fica salvo e volta igual na próxima vez que o Etapix abrir.

**Dicas:**
- F2 edita a célula selecionada; Tab vai para a coluna seguinte; Delete exclui as tarefas
  selecionadas; Insert insere uma tarefa.
- Alt+Shift+→ recua e Alt+Shift+← avança a tarefa.
- Ctrl+Z desfaz e Ctrl+Y refaz cada edição.
- Dois cliques na coluna **Modo da Tarefa** alternam entre manual e automático.
- Uma data digitada em **Início** vira a restrição "Não iniciar antes de", como no Project.

**Limitações e permissões:** a coluna **Término** não é editável (mude a duração). A coluna
**Adicionar Nova Coluna** ainda não adiciona colunas. A altura vale para todas as linhas.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Aba Tarefa

**Para que serve:** montar e ajustar o cronograma: inserir, organizar, vincular e marcar o
andamento das tarefas.

**O que você pode fazer aqui:**
- Inserir tarefa, resumo e marco (grupo **Inserir**).
- Recuar, avançar, vincular e desvincular (grupo **Cronograma**).
- Marcar 0%, 25%, 50%, 75% ou 100% concluído.
- Agendar manualmente ou automaticamente, mover tarefas e respeitar vínculos (grupo **Tarefas**).
- Recortar, copiar e colar tarefas (grupo **Área de Transferência**).
- Escolher a fonte e o tamanho do texto (grupo **Fonte**).
- Ver informações, adicionar à linha do tempo, rolar até a tarefa e localizar (grupos
  **Propriedades** e **Editar**).

**Procedimento: Montar uma fase com subtarefas**
1. Selecione as tarefas que formam a fase (clique na primeira e Shift+clique na última).
2. Clique em **Inserir > Resumo**.
3. Renomeie a linha **<Nova Tarefa Resumo>** com o nome da fase.

**Como conferir:** a fase aparece em negrito com um triângulo, as subtarefas ficam recuadas e o
Gantt mostra uma barra preta de resumo cobrindo-as.

**Se não funcionar:** se nada acontecer em **Recuar**, não há tarefa acima no mesmo nível; o
Etapix deixa a tarefa como está, como o Project.

**Procedimento: Vincular várias tarefas em sequência**
1. Selecione duas ou mais tarefas (Ctrl+clique ou Shift+clique).
2. Clique em **Vincular as Tarefas** (ícone de corrente, grupo **Cronograma**).

**Como conferir:** cada tarefa passa a ter a anterior em **Predecessoras**.

**Procedimento: Marcar o andamento**
1. Selecione a tarefa.
2. Clique no botão de porcentagem desejado. **Marcar no Controle** marca 100%.

**Como conferir:** uma faixa escura aparece dentro da barra; em 100%, um ✓ verde aparece na
coluna de indicadores.

**Dicas:**
- **Mover** desloca a data de início em dias corridos (1 dia, 1 semana, 2 semanas ou 4 semanas,
  para frente ou para trás) e grava a restrição "Não iniciar antes de".
- **Respeitar Vínculos** traz uma tarefa manual de volta para a data que seus vínculos indicam.
- **Limpar** exclui as tarefas selecionadas.

**Limitações e permissões:** precisa de tarefa selecionada para a maioria dos comandos. Em cinza
nesta versão: Pincel de Formatação, Negrito/Itálico/Sublinhado, Inativar, Inspecionar, Modo,
Entregável, Anotações, Detalhes e Preencher.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Arquivo

**Para que serve:** criar, abrir e salvar projetos.

**O que você pode fazer aqui:**
- **Novo**, **Abrir**, **Salvar** e **Salvar como**.
- Abrir, criar e salvar `.mpp` em qualquer máquina, sem o Project, mantendo no arquivo o que só o
  Project usa (modos de exibição, formatações). Abrir e salvar Project XML (`.xml`).

**Procedimento: Abrir um `.mpp`**
1. Clique em **Arquivo > Abrir**.
2. Escolha o arquivo `.mpp` e confirme.
3. O Etapix lê o `.mpp` sozinho, sem o Microsoft Project.

**Como conferir:** o nome do arquivo aparece no título da janela e as tarefas na grade.

**Se não funcionar:** "não foi possível ler o .mpp" indica um arquivo anterior ao Project 2010 ou
corrompido: abra-o no Project e salve de novo, ou peça o arquivo em `.xml`. Ao salvar, um erro
"(o arquivo está aberto no Project?)" indica que outro programa está com o arquivo aberto: feche-o
lá e tente de novo.

**Procedimento: Salvar**
1. Clique em **Salvar** (ou Ctrl+S, ou o disquete ao lado do logo).
2. Na primeira vez, escolha o nome e o formato (`.mpp` ou `.xml`).

**Como conferir:** o `*` some do título da janela.

**Dicas:**
- Ao fechar o Etapix, ou usar Novo e Abrir com alterações não salvas, ele pergunta se deseja
  salvar.
- O `.mpp` é gravado primeiro num arquivo temporário: se a conversão falhar, o arquivo original
  não é alterado.

**Limitações e permissões:** arquivos anteriores ao Project 2010 não abrem. Imprimir,
Exportar, Compartilhar e Opções ainda não existem.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Aba Recurso

**Para que serve:** cadastrar quem executa as tarefas, atribuir essas pessoas e resolver
superalocações.

**O que você pode fazer aqui:**
- **Adicionar Recursos** (recurso de trabalho, 100%).
- **Atribuir Recursos** à tarefa selecionada.
- **Nivelar Seleção**, **Nivelar Recurso**, **Nivelar Tudo** e **Limpar Nivelamento**.

**Procedimento: Atribuir recursos**
1. Selecione a tarefa.
2. Clique em **Atribuir Recursos**.
3. Marque as pessoas na lista.

**Como conferir:** os nomes aparecem na coluna **Nomes dos Recursos** e ao lado da barra no Gantt.

**Se não funcionar:** se a lista estiver vazia, cadastre antes em **Adicionar Recursos**, ou
digite os nomes direto na coluna **Nomes dos Recursos** (separados por vírgula). Tarefas de
resumo não recebem recursos.

**Procedimento: Resolver superalocação**
1. Clique em **Nivelar Tudo**.
2. Confira as datas que mudaram.
3. Se não gostar, use **Limpar Nivelamento** ou Ctrl+Z.

**Como conferir:** tarefas que disputavam o mesmo recurso passam a ficar em sequência.

**Dicas:** o nivelamento usa as opções padrão do Project e grava os atrasos de nivelamento no
arquivo.

**Limitações e permissões:** nesta versão, os três botões de nivelar nivelam o projeto inteiro.
Em cinza: Organizador de Equipe, Pool de Recursos, Informações do recurso, Anotações, Detalhes,
Opções de Nivelamento e Próxima Superalocação. Taxas e calendários de recurso vêm só de arquivos
abertos.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Aba Projeto

**Para que serve:** ajustar o projeto inteiro: data de início, linha de base e recálculo.

**O que você pode fazer aqui:**
- **Informações do Projeto** (data de início).
- **Definir Linha de Base**.
- **Mover Projeto** (nova data de início).
- **Calcular Projeto**.

**Procedimento: Definir a linha de base**
1. Termine de planejar.
2. Clique em **Definir Linha de Base**.
3. Para ver a comparação, marque **Formato > Linha de Base**.

**Como conferir:** uma barra cinza aparece sob cada tarefa no Gantt.

**Se não funcionar:** definir de novo substitui a linha de base anterior. Use Ctrl+Z para voltar.

**Dicas:** **Mover Projeto** desloca todas as tarefas automáticas; as manuais e as restrições de
data ficam onde estão.

**Limitações e permissões:** em cinza: Subprojeto, Suplementos, Campos Personalizados, Vínculos
Entre Projetos, EAP, Alterar Período de Trabalho, Data de Status, Atualizar Projeto e Ortografia.
Só a linha de base 0 é gravada.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Abas Exibir e Formato

**Para que serve:** escolher o que ver e como: modo de exibição, zoom, escala de tempo e estilos
das barras.

**O que você pode fazer aqui:**
- **Gráfico de Gantt** ou **Planilha de Tarefas**; **Planilha de Recursos** (abre o relatório
  Recursos).
- **Escala de Tempo** (Horas, Dias, Semanas, Meses), **Ampliar**, **Reduzir**, **Projeto
  Inteiro** e **Tarefas Selecionadas**.
- Mostrar ou ocultar a **Linha do Tempo**.
- Em Formato: **Tarefas Críticas**, **Margem de Atraso**, **Linha de Base** e **Número da
  Estrutura de Tópicos**.

**Procedimento: Ver o caminho crítico**
1. Abra a aba **Formato**.
2. Marque **Tarefas Críticas**.

**Como conferir:** as tarefas sem folga ficam vermelhas no Gantt.

**Dicas:** Shift + roda do mouse rola o Gantt na horizontal; arrastar a divisa entre a grade e o
Gantt muda o espaço de cada um.

**Limitações e permissões:** em cinza: Uso da Tarefa, Diagrama de Rede, Calendário, Organizador
de Equipe, Uso do Recurso, Classificar, Estrutura de Tópicos, Tabelas, Realce, Filtro, Agrupar
por, Detalhes, Janela, Macros, Estilos de Texto, Linhas de Grade, Layout, colunas e Desenho.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Aba Relatório

**Para que serve:** resumir o projeto para acompanhamento.

**O que você pode fazer aqui:**
- **Painéis:** início, término, tarefas, concluídas, folga negativa, recursos e custo total.
- **Recursos:** unidades, tarefas e custo de cada recurso.
- **Custos:** custo por tarefa e total.
- **Em Andamento:** tarefas iniciadas e atrasadas.

**Procedimento: Ver o que está atrasado**
1. Clique em **Em Andamento**.
2. Confira a coluna **Situação**.

**Como conferir:** tarefas com término antes de hoje e menos de 100% aparecem como **Atrasada**.

**Se não funcionar:** a lista vazia indica que nenhuma tarefa foi iniciada nem está atrasada.

**Limitações e permissões:** os relatórios não são impressos nem exportados. Em cinza:
Introdução, Personalizado, Relatórios Recentes, Comparar Projetos e Relatórios Visuais. Custos
só aparecem quando os recursos têm taxas, custo por uso ou atribuições de custo, ou quando as tarefas têm custo fixo (dados lidos e preservados em arquivos `.mpp` e `.xml`).

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Linha do Tempo

**Para que serve:** mostrar de relance o início, o término e as entregas principais.

**Procedimento: Colocar tarefas na linha do tempo**
1. Selecione as tarefas.
2. Clique em **Tarefa > Adicionar à Linha do Tempo**.

**Como conferir:** barras com o nome das tarefas aparecem na faixa acima da grade.

**Limitações e permissões:** a escolha não é salva no arquivo e vale só enquanto o Etapix estiver
aberto. Clicar de novo em **Adicionar à Linha do Tempo** tira a tarefa.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

### Etapix no Claude e no Codex

**Para que serve:** pedir a um assistente que crie ou altere cronogramas do Etapix.

**O que você pode fazer aqui:**
- Criar projetos, adicionar e editar tarefas, nivelar, definir a linha de base e salvar em
  `.mpp` ou `.xml`, pedindo em texto.

**Procedimento: Pedir um cronograma**
1. Instale o Etapix pelo `Etapix-Instalador.exe` (ou o pacote Linux): o conector é registrado no
   Claude Code e no Codex. No Claude Desktop, clique em **Instalar** na janela que se abre.
2. Reinicie o assistente.
3. Peça, por exemplo: "Crie um cronograma de implantação começando em 02/11/2026 em
   C:\Projetos\implantacao.mpp".
4. Abra o arquivo no Etapix.

**Como conferir:** o assistente informa os ids criados e a data de término; o arquivo abre no
Etapix e no Project.

**Se não funcionar:** se o assistente disser que não tem as ferramentas do Etapix, reinicie-o; se
continuar, reinstale o Etapix.

**Limitações e permissões:** o conector funciona no Claude Code, no Codex e no Claude Desktop.
No claude.ai pelo navegador, só a skill está disponível, sem o conector. O assistente altera o
arquivo diretamente: salve antes o que estiver aberto no Etapix.

**Responsável / revisão:** `[A DEFINIR]`, versão 1.0, 22/10/2026.

## 6. Tour guiado

### Apresentação de cada etapa

- Contador **Passo X de Y**.
- Título curto e uma ideia por etapa.
- Destaque: o retângulo do controle recebe borda esmeralda de 2 px; o resto da tela fica com um
  véu azul-marinho a 45% de opacidade, desenhado numa camada `egui::Order::Foreground`.
- Balão com **Anterior**, **Próximo** e **Pular tour**; na última etapa, **Concluir**.
- Etapa central (sem alvo) para a introdução e para alvos ausentes.

### Sequências

**Tour da grade e do Gantt** (primeira abertura do Etapix):
1. (Central) "O Etapix organiza o projeto em tarefas: à esquerda a grade, à direita o Gantt."
2. Linha vazia da grade: "Digite o nome da tarefa aqui e pressione Enter."
3. Coluna **Duração**: "Dias úteis: 3, 2,5 ou 0 para um marco."
4. Coluna **Predecessoras**: "O número da tarefa anterior, como no Project."
5. Gráfico de Gantt: "As barras se movem sozinhas quando você muda durações e vínculos."
6. Barra de acesso rápido: "Salvar, Desfazer e Refazer ficam sempre aqui."
7. Botão **Como usar**: "Os procedimentos completos ficam aqui."

**Tour da aba Tarefa:**
1. Grupo **Inserir**: tarefa, resumo e marco.
2. Grupo **Cronograma**: porcentagens, recuar, avançar e vincular.
3. Grupo **Tarefas**: manual ou automático, Mover.
4. Grupo **Propriedades**: Informações e Linha do Tempo.
5. Botão **Como usar**.

Sequências equivalentes (3 a 5 etapas) para Recurso, Projeto, Exibir, Formato e Relatório,
apresentando só os grupos com comandos ativos.

### Regras de funcionamento

- **Preparar antes de mostrar:** cada etapa declara a aba (`Tab`) e, se for o caso, o modo de
  exibição (`View::Gantt`). O controlador muda a aba e espera um quadro antes de procurar o alvo.
- **Encontrar o alvo:** o egui não tem DOM. Cada controle apresentável registra o próprio
  retângulo via `App::target` num mapa `targets: HashMap<&'static str, Rect>`, a cada quadro, com um id estável
  (veja a seção 8). O tour lê esse mapa no quadro seguinte.
- **Rolagem e redimensionamento:** como o mapa é atualizado a cada quadro, o destaque acompanha.
  Se o alvo estiver fora da área visível (faixa de opções rolada, grade rolada), o controlador
  rola até ele antes de mostrar.
- **Alvo ausente:** cada etapa declara `skip` (omitir) ou `center` (mostrar no centro, explicando).
  Na faixa compacta, os botões existem só como ícone, e o destaque vale para o ícone.
- O contador conta só as etapas disponíveis. Sem nenhuma etapa disponível, o tour abre o painel
  **Como usar** e mostra "Esta tela não tem tour; veja o passo a passo".
- Escape fecha o tour; as setas ← e → navegam, exceto quando uma célula está em edição.
- Mudar de aba manualmente durante o tour encerra o tour e remove o véu.
- O tour nunca cria, edita, exclui, nivela ou salva nada. Os cliques no balão só navegam entre as
  etapas; os cliques fora do balão são bloqueados enquanto o véu estiver visível.

Um treinamento com projeto de exemplo (opcional) deve abrir um arquivo de demonstração separado,
nunca o projeto do usuário, e avisar isso no início.

## 7. Primeira visita, progresso e versões

- Na primeira abertura do Etapix, e na primeira vez de cada aba, mostrar uma faixa discreta
  abaixo da faixa de opções: "Primeira vez aqui? **Fazer o tour** · **Ver passo a passo** ·
  **Dispensar**". A faixa não bloqueia o uso.
- Não repetir a sugestão de uma aba depois de concluída ou dispensada.
- Guardar no arquivo de configurações do usuário, em chaves separadas:
  - `ajuda_sugerir=1|0` (preferência geral);
  - `tour.<id>=nao_iniciado|em_andamento:<etapa>|concluido:<versao>|dispensado:<versao>`.
- Cada tutorial tem `version`. Quando a versão sobe por mudança relevante (por exemplo, um
  comando que deixa de ficar em cinza), a sugestão reaparece uma vez.
- Na retomada, conferir que a etapa salva ainda existe; se não, recomeçar do início.
- Os testes de interface usam um arquivo de configurações próprio (já é assim em `fonts.rs`): o
  tour não pode gravar nas configurações reais durante `cargo test`.

Concluir um tour não comprova treinamento nem leitura dos procedimentos.

## 8. Organização técnica recomendada

| Componente | Onde no Etapix | Responsabilidade |
|---|---|---|
| Registro de tutoriais | `app/src/help/content.rs` | Conteúdo em constantes Rust (`TUTORIALS`, `CATALOG`, `START`): módulos, versões, procedimentos e etapas. Sem leitura de arquivos externos, para não executar conteúdo arbitrário. |
| Catálogo | `app/src/help/mod.rs` (`App::help_catalog`), aberto por **Ajuda > Central de Ajuda** | Cartões por grupo, **Ir para** e **Como usar**. |
| Painel de ajuda | `app/src/help/mod.rs` (`App::help_panel`) | `SidePanel::right` (ou janela flutuante abaixo de 900 px) com os procedimentos expansíveis. |
| Controlador do tour | `app/src/help/mod.rs` (`App::help_tour`, `App::help_banner`) | Etapa atual, preparação de aba, alvo ausente, encerramento e faixa de sugestão na primeira visita. |
| Camadas visuais | Mesmo módulo, `egui::Area` em `Order::Foreground` | Véu, destaque e balão. |
| Alvos | `ribbon.rs`, `main.rs` (`App::target`) | Cada controle registra seu retângulo visível em `self.help.targets` a cada quadro. |
| Progresso e preferências | `fonts::setting` / `fonts::save_settings` (já existem) | Chaves `ajuda_sugerir` e `tour.<id>`. |

Ids estáveis no formato `aba.grupo[.controle]`, independentes de texto e posição. Por exemplo:
`grade.linha_vazia`, `grade.duracao`, `grade.predecessoras`, `gantt`, `acesso_rapido`,
`tarefa.inserir`, `tarefa.cronograma`, `recurso.nivel`, `projeto.linha_de_base`,
`exibir.zoom`, `formato.estilos_barra`, `ajuda.como_usar`.

### Exemplo de configuração

Ilustrativo: no Etapix o conteúdo fica em Rust, mas o formato é o mesmo.

```json
{
  "id": "tarefa",
  "version": 1,
  "tab": "Task",
  "title": "Tarefa",
  "summary": "Insira, organize, vincule e marque o andamento das tarefas.",
  "capabilities": ["Inserir tarefa, resumo e marco", "Recuar e vincular", "Marcar % concluída"],
  "procedures": [
    {
      "id": "fase",
      "title": "Montar uma fase com subtarefas",
      "steps": [
        "Selecione as tarefas da fase (clique na primeira e Shift+clique na última).",
        "Clique em Inserir > Resumo.",
        "Renomeie a linha <Nova Tarefa Resumo> com o nome da fase."
      ],
      "expectedResult": "A fase aparece em negrito e as subtarefas ficam recuadas."
    }
  ],
  "tips": ["Ctrl+Z desfaz o resumo inteiro de uma vez."],
  "tour": [
    {
      "id": "inserir",
      "target": "tarefa.inserir",
      "title": "Inserir",
      "text": "Tarefa, resumo e marco entram abaixo da linha selecionada.",
      "onMissing": "skip"
    },
    {
      "id": "como-usar",
      "target": "ajuda.como_usar",
      "title": "Ajuda a qualquer momento",
      "text": "Os procedimentos completos ficam em Como usar.",
      "onMissing": "center"
    }
  ]
}
```

A preparação de cada etapa é só a troca de aba e de modo de exibição, por funções conhecidas do
App. Nenhum texto de conteúdo é interpretado como comando.

## 9. Acessibilidade e apresentação

- O egui expõe árvore de acessibilidade pelo AccessKit: dar nome a todos os botões só com ícone
  (hoje a dica de mouse já leva o nome; repetir no rótulo acessível).
- Painel e balão recebem o foco ao abrir; Tab circula dentro deles; ao fechar, o foco volta ao
  botão **Como usar**.
- Com o véu visível, os controles do fundo não recebem cliques nem foco.
- Tudo funciona pelo teclado: Enter e espaço acionam, Escape fecha, ← e → navegam no tour.
- O destaque não depende só da cor: borda grossa e o texto do balão dizem qual controle é.
- Sem animações no tour; ele só se move quando o alvo se move.
- Na janela estreita (faixa compacta), o balão fica dentro da janela e quebra o texto; nada é
  cortado.
- Popups (Mover, Escala de Tempo) e janelas (Atribuir Recursos, a pergunta de salvar) pausam o
  tour; ele continua quando fecham.

## 10. Critérios de aceitação

Situação em 22/10/2026. "Teste" indica um teste automático em `app/src/monkey_tests.rs`;
"imagem" indica conferência pelo modo de captura (`ETAPIX_SNAPSHOT_DIR`).

- [x] Grade, Arquivo e cada aba da faixa de opções têm **Como usar** (teste
      `help_panel_and_catalog_open_from_every_tab`; Arquivo pelo catálogo).
- [x] A Central de Ajuda abre a aba e a seção corretas (**Ir para** troca a aba; **Como usar**
      abre o procedimento do cartão; imagem).
- [x] Os procedimentos dizem ação, confirmação e resultado esperado. Os textos foram escritos a
      partir do comportamento desta versão; falta a revisão do responsável.
- [x] Os tours da grade e de cada aba vão do início ao fim e podem ser refeitos (teste
      `every_tour_runs_to_the_end_without_touching_the_project`).
- [x] Próximo (seta →) e Concluir levam cada tour até o fim (teste); Anterior, Próximo e Pular
      tour aparecem no balão (imagem). Anterior desativado no primeiro passo e o clique em cada
      botão: só no código, sem teste.
- [x] Alvo ausente não trava o tour: com a coluna Predecessoras fora da tela, o tour da grade
      pula esse passo e segue (teste). O contador desconta os passos pulados (código; não
      conferido em imagem). Projeto vazio: a linha vazia existe sempre.
- [x] A troca de aba acontece antes de procurar o alvo (o tour espera até dois quadros).
- [ ] Rolar até o alvo: não implementado. Um alvo fora da área visível é pulado (`skip`) ou o
      passo aparece no centro (`center`), em vez de a grade ou a faixa rolarem até ele.
- [x] Escape encerra o tour a qualquer momento (teste); Escape fecha o painel quando não há
      célula em edição.
- [ ] Foco do teclado no painel e no balão e retorno do foco ao botão: não verificado.
- [ ] Janela de 1024 × 600 e zoom do Windows a 150%: não verificado. Abaixo de 900 px de largura o
      painel vira janela flutuante (código), sem teste em tela.
- [x] O cartão de `.mpp` adapta o texto no Linux informando que `.mpp` abre e salva como no Windows sem o Project (código; o Linux compila e roda os testes no
      GitHub).
- [x] Sugestão, dispensa e conclusão ficam por usuário e por versão (chaves `tour.<id>` e
      `ajuda_sugerir`); os testes usam um arquivo de configurações próprio.
- [x] O tour não altera o projeto: o teste roda todos os tours e confere que nada mudou.
- [x] O usuário simulado (32.000 ações na última bateria) também esbarra na sugestão, no botão
      **?** da grade, em **Como usar** e no tour, sem travar.
- [ ] O responsável revisou o conteúdo.

## 11. Manutenção e indicadores

- Cada vez que um comando sair do cinza, revisar a seção **Limitações** do módulo e subir a
  versão do tutorial.
- Incluir a revisão dos tutoriais no mesmo commit da mudança de interface.
- Indicadores: o Etapix não envia dados para fora. Se for útil, registrar só contadores locais
  (início, conclusão, dispensa, etapa abandonada), sem nomes de tarefas nem conteúdo dos
  projetos, e nunca enviá-los automaticamente.

## 12. Prompt pronto para aplicar no Etapix

```text
[Papel]
Atue como desenvolvedor Rust/egui e especialista em experiência de uso e documentação.

[Contexto]
A aplicação é o Etapix (repositório C:\dev\pjsched, app/), planejador de projetos com a
interface do Microsoft Project, para a equipe interna. Interface em egui/eframe 0.29.
Use docs/GUIA_TUTORIAIS_ETAPIX.md como especificação funcional.

[Tarefa]
Implemente a Central de Ajuda, o painel Como usar por aba e o tour guiado, começando pela
grade de tarefas e pela aba Tarefa, depois Arquivo, Recurso, Projeto, Exibir, Formato e
Relatório.

[Formato]
Entregue o código integrado (app/src/help/), o conteúdo em Rust, os alvos registrados nos
controles, testes em monkey_tests.rs e um relatório curto com alterações, verificações,
pendências e melhorias.

[Restrições]
Reutilize as constantes de cor, fontes, icons.rs e fonts::setting. Não acrescente
dependências. Use ids estáveis de alvo. O tour não pode alterar o projeto nem gravar nas
configurações reais durante os testes. Mantenha o build para Windows e Linux.

[Revisões]
Rode cargo test (incluindo o usuário simulado) e o modo de captura ETAPIX_SNAPSHOT_DIR.
Separe o que foi observado do que não foi verificado. Não declare testes não executados.

[Tom]
Português claro e direto, com os nomes exatos da interface do Etapix.
```

## Pendências

- Definir o responsável pela revisão do conteúdo e fazer a revisão.
- Rolar a grade ou a faixa até um alvo fora da tela, em vez de pulá-lo.
- Foco do teclado e rótulos acessíveis (AccessKit) no painel e no balão.
- Conferir janela pequena e zoom do Windows em tela real.
- Pausar o tour quando um menu (Mover, Escala de Tempo) ou uma janela se abrir: hoje o véu
  simplesmente bloqueia o fundo.

## Pontos de melhoria

- Abrir a ajuda direto no assunto ao clicar com o botão direito num comando.
- Um projeto de exemplo para treinar sem mexer em arquivos reais.
- Imagens geradas pelo modo de captura (`ETAPIX_SNAPSHOT_DIR`) nos procedimentos.
- Revisar as etapas mais abandonadas, se os contadores locais forem adotados.
