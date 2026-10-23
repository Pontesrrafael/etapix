//! What the help says: one tutorial per screen, with its procedures and its tour. Kept as Rust
//! constants so nothing read from a file is ever run (docs/GUIA_TUTORIAIS_ETAPIX.md, section 8).
//! The names match the interface of this version; when a greyed-out command starts working,
//! update its tutorial's limitations and raise `version`.

use crate::icons::Icon;
use crate::Tab;

pub struct Procedure {
    pub title: &'static str,
    pub steps: &'static [&'static str],
    pub check: &'static str,
    pub fails: &'static str,
}

/// What the tour does when a step's control is not on screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Skip,
    Center,
}

pub struct Step {
    /// The control to highlight (see `App::target`); `None` for a centred step.
    pub target: Option<&'static str>,
    /// The tab to open before looking for the control.
    pub tab: Option<Tab>,
    pub title: &'static str,
    pub text: &'static str,
    pub missing: Missing,
}

pub struct Tutorial {
    pub id: &'static str,
    pub version: u32,
    pub icon: Icon,
    /// The ribbon tab the tutorial belongs to (`None`: the grid and Gantt, or no screen).
    pub tab: Option<Tab>,
    pub title: &'static str,
    pub summary: &'static str,
    pub capabilities: &'static [&'static str],
    pub procedures: &'static [Procedure],
    pub tips: &'static [&'static str],
    pub limits: &'static str,
    pub tour: &'static [Step],
}

const fn step(target: &'static str, tab: Option<Tab>, title: &'static str, text: &'static str, missing: Missing) -> Step {
    Step { target: Some(target), tab, title, text, missing }
}

const fn center(title: &'static str, text: &'static str) -> Step {
    Step { target: None, tab: None, title, text, missing: Missing::Center }
}

pub const TUTORIALS: &[Tutorial] = &[
    Tutorial {
        id: "grade",
        version: 1,
        icon: Icon::Gantt,
        tab: None,
        title: "Grade de tarefas e Gráfico de Gantt",
        summary: "Liste as tarefas com duração, datas, predecessoras e recursos, e veja o cronograma em barras ao lado.",
        capabilities: &[
            "Digitar tarefas novas nas linhas vazias",
            "Editar nome, duração, início, predecessoras e nomes dos recursos",
            "Ajustar a largura das colunas e a altura das linhas",
            "Selecionar várias tarefas (Ctrl e Shift) para os comandos da faixa de opções",
        ],
        procedures: &[
            Procedure {
                title: "Criar tarefas",
                steps: &[
                    "Clique na coluna Nome da Tarefa da primeira linha vazia.",
                    "Digite o nome e pressione Enter. O cursor desce para a linha seguinte.",
                    "Repita para as próximas tarefas.",
                    "Clique na coluna Duração, digite os dias úteis (por exemplo, 3 ou 2,5) e pressione Enter.",
                ],
                check: "A tarefa ganha um número na primeira coluna, as datas de Início e Término aparecem e uma barra surge no Gantt.",
                fails: "\"Duração inválida\" na barra de status: use só números (3, 2,5, 3 dias). Para cancelar uma edição, pressione Escape.",
            },
            Procedure {
                title: "Vincular tarefas pelas predecessoras",
                steps: &[
                    "Clique na coluna Predecessoras da tarefa que vem depois.",
                    "Digite o número da tarefa anterior. Para outros vínculos, acrescente TI (término-início), II (início-início), TT (término-término) ou IT (início-término) e uma defasagem opcional, como 2TI+2 dias ou 3II-1d. Separe várias com ponto e vírgula.",
                    "Pressione Enter.",
                ],
                check: "A data de início se move e uma seta liga as barras no Gantt.",
                fails: "\"predecessora inválida\": sintaxe fora do padrão acima. \"não existe tarefa N\": número que não está na grade. \"os vínculos formam um ciclo\": a tarefa depende dela mesma. Nada é alterado nesses casos.",
            },
            Procedure {
                title: "Ajustar colunas e linhas",
                steps: &[
                    "Arraste a divisa entre dois títulos de coluna, no cabeçalho. Dois cliques ajustam a coluna ao maior texto.",
                    "Arraste a divisa entre duas linhas, na coluna dos números.",
                ],
                check: "O tamanho fica salvo e volta igual na próxima vez que o Etapix abrir.",
                fails: "Se o cursor não virar ↔ ou ↕, aproxime o mouse da linha divisória.",
            },
        ],
        tips: &[
            "F2 edita a célula selecionada; Tab vai para a coluna seguinte; Delete exclui as tarefas selecionadas; Insert insere uma tarefa.",
            "Alt+Shift+→ recua e Alt+Shift+← avança a tarefa.",
            "Ctrl+Z desfaz e Ctrl+Y refaz cada edição.",
            "Dois cliques na coluna Modo da Tarefa alternam entre manual e automático.",
            "Uma data digitada em Início vira a restrição \"Não iniciar antes de\", como no Project.",
        ],
        limits: "A coluna Término não é editável (mude a duração). Adicionar Nova Coluna ainda não adiciona colunas. A altura vale para todas as linhas.",
        tour: &[
            center("Bem-vindo ao Etapix", "O Etapix organiza o projeto em tarefas: à esquerda a grade, à direita o Gráfico de Gantt."),
            step("grade.linha_vazia", None, "Digite aqui", "Clique numa linha vazia, digite o nome da tarefa e pressione Enter.", Missing::Center),
            step("grade.duracao", None, "Duração", "Dias úteis: 3, 2,5 ou 0 para um marco.", Missing::Skip),
            step("grade.predecessoras", None, "Predecessoras", "O número da tarefa anterior, como no Project. Também aceita 3II ou 2TI+2 dias.", Missing::Skip),
            step("gantt", None, "Gráfico de Gantt", "As barras se movem sozinhas quando você muda durações e vínculos.", Missing::Skip),
            step("acesso_rapido", None, "Salvar, Desfazer, Refazer", "Ficam sempre aqui, ao lado do nome Etapix.", Missing::Skip),
            step("ajuda.como_usar", Some(Tab::Task), "Ajuda a qualquer momento", "Os procedimentos completos ficam em Como usar, no fim de cada aba.", Missing::Center),
        ],
    },
    Tutorial {
        id: "tarefa",
        version: 1,
        icon: Icon::Task,
        tab: Some(Tab::Task),
        title: "Tarefa",
        summary: "Monte e ajuste o cronograma: insira, organize, vincule e marque o andamento das tarefas.",
        capabilities: &[
            "Inserir tarefa, resumo e marco (grupo Inserir)",
            "Recuar, avançar, vincular e desvincular (grupo Cronograma)",
            "Marcar 0%, 25%, 50%, 75% ou 100% concluído",
            "Agendar manual ou automaticamente, mover tarefas e respeitar vínculos (grupo Tarefas)",
            "Recortar, copiar e colar tarefas (grupo Área de Transferência)",
            "Escolher a fonte e o tamanho do texto (grupo Fonte)",
            "Ver informações, adicionar à linha do tempo, rolar até a tarefa e localizar",
        ],
        procedures: &[
            Procedure {
                title: "Montar uma fase com subtarefas",
                steps: &[
                    "Selecione as tarefas da fase (clique na primeira e Shift+clique na última).",
                    "Clique em Inserir > Resumo.",
                    "Renomeie a linha <Nova Tarefa Resumo> com o nome da fase.",
                ],
                check: "A fase aparece em negrito com um triângulo, as subtarefas ficam recuadas e o Gantt mostra uma barra preta de resumo.",
                fails: "Se Recuar não fizer nada, não há tarefa acima no mesmo nível; o Etapix deixa a tarefa como está, como o Project.",
            },
            Procedure {
                title: "Vincular várias tarefas em sequência",
                steps: &["Selecione duas ou mais tarefas (Ctrl+clique ou Shift+clique).", "Clique em Vincular as Tarefas (ícone de corrente, grupo Cronograma)."],
                check: "Cada tarefa passa a ter a anterior em Predecessoras.",
                fails: "O botão fica em cinza com menos de duas tarefas selecionadas.",
            },
            Procedure {
                title: "Marcar o andamento",
                steps: &["Selecione a tarefa.", "Clique na porcentagem desejada. Marcar no Controle marca 100%."],
                check: "Uma faixa escura aparece dentro da barra; em 100%, um ✓ verde aparece na coluna de indicadores.",
                fails: "Tarefas de resumo calculam o andamento pelas subtarefas: marque as subtarefas.",
            },
        ],
        tips: &[
            "Mover desloca o início em dias corridos (1 dia a 4 semanas, para frente ou para trás) e grava a restrição \"Não iniciar antes de\".",
            "Respeitar Vínculos traz uma tarefa manual de volta para a data que os vínculos indicam.",
            "Limpar exclui as tarefas selecionadas. Ctrl+Z desfaz.",
        ],
        limits: "A maioria dos comandos precisa de uma tarefa selecionada. Em cinza nesta versão: Pincel de Formatação, Negrito/Itálico/Sublinhado, Inativar, Inspecionar, Modo, Entregável, Anotações, Detalhes e Preencher.",
        tour: &[
            step("tarefa.inserir", Some(Tab::Task), "Inserir", "Tarefa, resumo e marco entram abaixo da linha selecionada.", Missing::Skip),
            step("tarefa.cronograma", Some(Tab::Task), "Cronograma", "Porcentagem concluída, recuar, avançar e vincular as tarefas selecionadas.", Missing::Skip),
            step("tarefa.tarefas", Some(Tab::Task), "Tarefas", "Agendamento manual ou automático e Mover.", Missing::Skip),
            step("tarefa.propriedades", Some(Tab::Task), "Propriedades", "Informações da tarefa e Adicionar à Linha do Tempo.", Missing::Skip),
            step("ajuda.como_usar", Some(Tab::Task), "Como usar", "O passo a passo completo desta aba fica aqui.", Missing::Center),
        ],
    },
    Tutorial {
        id: "arquivo",
        version: 1,
        icon: Icon::Sheet,
        tab: Some(Tab::File),
        title: "Arquivo",
        summary: "Crie, abra e salve projetos em .mpp e Project XML. Abrir .mpp e salvar alterações nas tarefas não precisam do Microsoft Project.",
        capabilities: &["Novo, Abrir, Salvar e Salvar como", "Abrir .mpp em qualquer máquina, sem o Microsoft Project", "Salvar no próprio .mpp, sem o Project, as alterações nas tarefas que já existem", "Incluir ou excluir tarefas, vínculos e recursos e salvar em .mpp, com o Microsoft Project instalado", "Abrir e salvar Project XML (.xml) em qualquer sistema"],
        procedures: &[
            Procedure {
                title: "Abrir um .mpp",
                steps: &["Clique em Arquivo > Abrir.", "Escolha o arquivo .mpp e confirme.", "O Etapix lê o .mpp sozinho: não precisa do Microsoft Project."],
                check: "O nome do arquivo aparece no título da janela e as tarefas, na grade.",
                fails: "\"não foi possível ler o .mpp\": o arquivo é de uma versão anterior ao Project 2010 ou está corrompido. Abra-o no Project e salve de novo, ou peça o arquivo em .xml.",
            },
            Procedure {
                title: "Salvar",
                steps: &["Clique em Salvar (Ctrl+S ou o disquete ao lado do logo).", "Na primeira vez, escolha o nome e o formato (.mpp ou .xml)."],
                check: "O * some do título da janela.",
                fails: "Se o .mpp não salvar (incluir ou excluir tarefas, vínculos ou recursos ainda precisa do Microsoft Project), salve como .xml: o Project abre esse formato sem perder o cronograma.",
            },
        ],
        tips: &[
            "Ao fechar o Etapix, ou usar Novo e Abrir com alterações não salvas, ele pergunta se deseja salvar.",
            "O .mpp é gravado primeiro num arquivo temporário: se a conversão falhar, o arquivo original não muda.",
        ],
        limits: "Incluir ou excluir tarefas, vínculos, recursos ou calendários e salvar em .mpp exige o Microsoft Project instalado; abrir e salvar alterações nas tarefas, não. Imprimir, Exportar, Compartilhar e Opções ainda não existem.",
        tour: &[],
    },
    Tutorial {
        id: "recurso",
        version: 1,
        icon: Icon::Team,
        tab: Some(Tab::Resource),
        title: "Recurso",
        summary: "Cadastre quem executa as tarefas, atribua essas pessoas e resolva superalocações.",
        capabilities: &["Adicionar Recursos (recurso de trabalho, 100%)", "Atribuir Recursos à tarefa selecionada", "Nivelar Seleção, Nivelar Recurso, Nivelar Tudo e Limpar Nivelamento"],
        procedures: &[
            Procedure {
                title: "Atribuir recursos",
                steps: &["Selecione a tarefa.", "Clique em Atribuir Recursos.", "Marque as pessoas na lista."],
                check: "Os nomes aparecem em Nomes dos Recursos e ao lado da barra no Gantt.",
                fails: "Lista vazia: cadastre antes em Adicionar Recursos, ou digite os nomes direto na coluna Nomes dos Recursos, separados por vírgula. Tarefas de resumo não recebem recursos.",
            },
            Procedure {
                title: "Resolver superalocação",
                steps: &["Clique em Nivelar Tudo.", "Confira as datas que mudaram.", "Se não gostar, use Limpar Nivelamento ou Ctrl+Z."],
                check: "Tarefas que disputavam o mesmo recurso passam a ficar em sequência.",
                fails: "Sem recursos atribuídos, o nivelamento não muda nada.",
            },
        ],
        tips: &["O nivelamento usa as opções padrão do Project e grava os atrasos de nivelamento no arquivo."],
        limits: "Nesta versão, os três botões de nivelar nivelam o projeto inteiro. Em cinza: Organizador de Equipe, Pool de Recursos, Informações do recurso, Anotações, Detalhes, Opções de Nivelamento e Próxima Superalocação.",
        tour: &[
            step("recurso.atribuicoes", Some(Tab::Resource), "Atribuições", "Escolha quem trabalha na tarefa selecionada.", Missing::Skip),
            step("recurso.inserir", Some(Tab::Resource), "Adicionar Recursos", "Cadastre as pessoas do projeto.", Missing::Skip),
            step("recurso.nivel", Some(Tab::Resource), "Nível", "Resolva superalocações; Limpar Nivelamento desfaz.", Missing::Skip),
            step("ajuda.como_usar", Some(Tab::Resource), "Como usar", "O passo a passo completo desta aba fica aqui.", Missing::Center),
        ],
    },
    Tutorial {
        id: "projeto",
        version: 1,
        icon: Icon::Calc,
        tab: Some(Tab::Project),
        title: "Projeto",
        summary: "Ajuste o projeto inteiro: data de início, linha de base e recálculo.",
        capabilities: &["Informações do Projeto (data de início)", "Definir Linha de Base", "Mover Projeto (nova data de início)", "Calcular Projeto"],
        procedures: &[Procedure {
            title: "Definir a linha de base",
            steps: &["Termine de planejar.", "Clique em Definir Linha de Base.", "Para ver a comparação, marque Formato > Linha de Base."],
            check: "Uma barra cinza aparece sob cada tarefa no Gantt.",
            fails: "Definir de novo substitui a linha de base anterior. Use Ctrl+Z para voltar.",
        }],
        tips: &["Mover Projeto desloca todas as tarefas automáticas; as manuais e as restrições de data ficam onde estão."],
        limits: "Em cinza: Subprojeto, Suplementos, Campos Personalizados, Vínculos Entre Projetos, EAP, Alterar Período de Trabalho, Data de Status, Atualizar Projeto e Ortografia. Só a linha de base 0 é gravada.",
        tour: &[
            step("projeto.propriedades", Some(Tab::Project), "Informações do Projeto", "A data de início de todo o cronograma.", Missing::Skip),
            step("projeto.cronograma", Some(Tab::Project), "Cronograma", "Linha de base, recálculo e Mover Projeto.", Missing::Skip),
            step("ajuda.como_usar", Some(Tab::Project), "Como usar", "O passo a passo completo desta aba fica aqui.", Missing::Center),
        ],
    },
    Tutorial {
        id: "exibir",
        version: 1,
        icon: Icon::Whole,
        tab: Some(Tab::View),
        title: "Exibir",
        summary: "Escolha o que ver e como: modo de exibição, zoom e escala de tempo.",
        capabilities: &[
            "Gráfico de Gantt ou Planilha de Tarefas; Planilha de Recursos",
            "Escala de Tempo (Horas, Dias, Semanas, Meses), Ampliar e Reduzir",
            "Projeto Inteiro e Tarefas Selecionadas",
            "Mostrar ou ocultar a Linha do Tempo",
        ],
        procedures: &[Procedure {
            title: "Ver o projeto inteiro",
            steps: &["Clique em Projeto Inteiro, no grupo Zoom."],
            check: "Todas as barras cabem na largura do Gantt.",
            fails: "Num projeto muito longo, as barras ficam finas: use Tarefas Selecionadas para aproximar uma parte.",
        }],
        tips: &["Shift + roda do mouse rola o Gantt na horizontal; arrastar a divisa entre a grade e o Gantt muda o espaço de cada um."],
        limits: "Em cinza: Uso da Tarefa, Diagrama de Rede, Calendário, Organizador de Equipe, Uso do Recurso, Classificar, Estrutura de Tópicos, Tabelas, Realce, Filtro, Agrupar por, Detalhes, Janela e Macros.",
        tour: &[
            step("exibir.zoom", Some(Tab::View), "Zoom", "Escala de tempo, ampliar, reduzir e caber o projeto inteiro.", Missing::Skip),
            step("exibir.modo_divisao", Some(Tab::View), "Linha do Tempo", "Mostre ou oculte a faixa de resumo acima da grade.", Missing::Skip),
            step("ajuda.como_usar", Some(Tab::View), "Como usar", "O passo a passo completo desta aba fica aqui.", Missing::Center),
        ],
    },
    Tutorial {
        id: "formato",
        version: 1,
        icon: Icon::Format,
        tab: Some(Tab::Format),
        title: "Formato",
        summary: "Escolha os estilos das barras: caminho crítico, folga, linha de base e números da estrutura.",
        capabilities: &["Tarefas Críticas em vermelho", "Margem de Atraso (folga)", "Linha de Base", "Número da Estrutura de Tópicos"],
        procedures: &[Procedure {
            title: "Ver o caminho crítico",
            steps: &["Abra a aba Formato.", "Marque Tarefas Críticas."],
            check: "As tarefas sem folga ficam vermelhas no Gantt.",
            fails: "Se nada ficar vermelho, todas as tarefas têm folga: vincule-as para formar a sequência do projeto.",
        }],
        tips: &["Margem de Atraso desenha, depois de cada tarefa, quanto ela pode atrasar sem atrasar o projeto."],
        limits: "Em cinza: Estilos de Texto, Linhas de Grade, Layout, colunas, Formatar barras, Tarefas Atrasadas, Tarefa de Resumo do Projeto e Desenho.",
        tour: &[
            step("formato.estilos_barra", Some(Tab::Format), "Estilos de Barra", "Caminho crítico, folga e linha de base no Gantt.", Missing::Skip),
            step("ajuda.como_usar", Some(Tab::Format), "Como usar", "O passo a passo completo desta aba fica aqui.", Missing::Center),
        ],
    },
    Tutorial {
        id: "relatorio",
        version: 1,
        icon: Icon::Dashboard,
        tab: Some(Tab::Report),
        title: "Relatório",
        summary: "Resuma o projeto para acompanhamento: painel, recursos, custos e andamento.",
        capabilities: &[
            "Painéis: início, término, tarefas, concluídas, folga negativa, recursos e custo",
            "Recursos: unidades, tarefas e custo de cada recurso",
            "Custos: custo por tarefa e total",
            "Em Andamento: tarefas iniciadas e atrasadas",
        ],
        procedures: &[Procedure {
            title: "Ver o que está atrasado",
            steps: &["Clique em Em Andamento.", "Confira a coluna Situação."],
            check: "Tarefas com término antes de hoje e menos de 100% aparecem como Atrasada.",
            fails: "Lista vazia: nenhuma tarefa foi iniciada nem está atrasada.",
        }],
        tips: &["Custos só aparecem quando os recursos têm taxas ou as tarefas têm custo fixo, o que vem de arquivos criados no Project."],
        limits: "Os relatórios não são impressos nem exportados. Em cinza: Introdução, Personalizado, Relatórios Recentes, Comparar Projetos e Relatórios Visuais.",
        tour: &[
            step("relatorio.exibir", Some(Tab::Report), "Relatórios", "Painéis, Recursos, Custos e Em Andamento.", Missing::Skip),
            step("ajuda.como_usar", Some(Tab::Report), "Como usar", "O passo a passo completo desta aba fica aqui.", Missing::Center),
        ],
    },
    Tutorial {
        id: "linha_do_tempo",
        version: 1,
        icon: Icon::TimelineAdd,
        tab: Some(Tab::Task),
        title: "Linha do Tempo",
        summary: "Mostre de relance o início, o término e as entregas principais.",
        capabilities: &["Colocar e tirar tarefas da faixa acima da grade", "Mostrar ou ocultar a faixa (Exibir > Linha do Tempo)"],
        procedures: &[Procedure {
            title: "Colocar tarefas na linha do tempo",
            steps: &["Selecione as tarefas.", "Clique em Tarefa > Adicionar à Linha do Tempo."],
            check: "Barras com o nome das tarefas aparecem na faixa acima da grade.",
            fails: "Clicar de novo em Adicionar à Linha do Tempo tira a tarefa.",
        }],
        tips: &[],
        limits: "A escolha não é salva no arquivo: vale enquanto o Etapix estiver aberto.",
        tour: &[],
    },
    Tutorial {
        id: "assistentes",
        version: 1,
        icon: Icon::Help,
        tab: None,
        title: "Etapix no Claude e no Codex",
        summary: "Peça a um assistente que crie ou altere cronogramas do Etapix.",
        capabilities: &["Criar projetos, adicionar e editar tarefas, nivelar, definir a linha de base e salvar em .mpp ou .xml, pedindo em texto"],
        procedures: &[Procedure {
            title: "Pedir um cronograma",
            steps: &[
                "Instale o Etapix pelo Etapix-Instalador.exe (ou o pacote Linux): o conector é registrado no Claude Code e no Codex. No Claude Desktop, clique em Instalar na janela que se abre.",
                "Reinicie o assistente.",
                "Peça, por exemplo: \"Crie um cronograma de implantação começando em 02/11/2026 em C:\\Projetos\\implantacao.mpp\".",
                "Abra o arquivo no Etapix.",
            ],
            check: "O assistente informa os ids criados e a data de término; o arquivo abre no Etapix e no Project.",
            fails: "Se o assistente disser que não tem as ferramentas do Etapix, reinicie-o; se continuar, reinstale o Etapix.",
        }],
        tips: &["Salve o que estiver aberto no Etapix antes de pedir mudanças no mesmo arquivo: o assistente grava direto nele."],
        limits: "O conector funciona no Claude Code, no Codex e no Claude Desktop (Windows). No claude.ai pelo navegador, só a skill.",
        tour: &[],
    },
];

pub fn tutorial(id: &str) -> Option<&'static Tutorial> {
    TUTORIALS.iter().find(|t| t.id == id)
}

/// The tutorial for a ribbon tab (the grid's when the tab has none of its own).
pub fn for_tab(tab: Tab) -> &'static Tutorial {
    let id = match tab {
        Tab::Task => "tarefa",
        Tab::Resource => "recurso",
        Tab::Report => "relatorio",
        Tab::Project => "projeto",
        Tab::View => "exibir",
        Tab::Format => "formato",
        Tab::File => "arquivo",
        Tab::Help => "grade",
    };
    tutorial(id).expect("every tab has a tutorial")
}

/// A card of the Central de Ajuda: it opens a tutorial (at one of its procedures) and, with
/// "Ir para", its tab.
pub struct Card {
    pub icon: Icon,
    pub title: &'static str,
    pub benefit: &'static str,
    pub tutorial: &'static str,
    pub procedure: usize,
    /// Only on Windows (`.mpp`, Claude Desktop).
    pub windows_only: bool,
}

const fn card(icon: Icon, title: &'static str, benefit: &'static str, tutorial: &'static str, procedure: usize) -> Card {
    Card { icon, title, benefit, tutorial, procedure, windows_only: false }
}

pub const CATALOG: &[(&str, &[Card])] = &[
    (
        "Montar o cronograma",
        &[
            card(Icon::Task, "Tarefas e duração", "Digite as tarefas e quantos dias úteis cada uma leva.", "grade", 0),
            card(Icon::Summary, "Estrutura", "Agrupe tarefas em fases com resumos e subtarefas.", "tarefa", 0),
            card(Icon::Link, "Vínculos e predecessoras", "Diga o que depende do quê, com o tipo e a defasagem.", "grade", 1),
            card(Icon::Milestone, "Marcos", "Marque entregas com duração 0 ou Inserir > Marco.", "grade", 0),
        ],
    ),
    (
        "Pessoas e custos",
        &[
            card(Icon::Assign, "Recursos e atribuições", "Cadastre as pessoas e diga quem faz cada tarefa.", "recurso", 0),
            card(Icon::Level, "Nivelamento", "Resolva quem está em duas tarefas ao mesmo tempo.", "recurso", 1),
            card(Icon::Cost, "Custos", "Veja o custo de cada tarefa e do projeto.", "relatorio", 0),
        ],
    ),
    (
        "Acompanhar",
        &[
            card(Icon::Percent(50), "% concluída", "Registre o andamento de cada tarefa.", "tarefa", 2),
            card(Icon::Baseline, "Linha de base", "Guarde o plano para comparar com o realizado.", "projeto", 0),
            card(Icon::InProgress, "Relatórios", "Painel, recursos, custos, andamento e atrasos.", "relatorio", 0),
            card(Icon::TimelineAdd, "Linha do tempo", "Mostre as entregas principais numa faixa de resumo.", "linha_do_tempo", 0),
        ],
    ),
    (
        "Arquivos",
        &[
            Card {
                icon: Icon::Sheet,
                title: "Abrir e salvar .mpp e .xml",
                benefit: "Troque arquivos com quem usa o Microsoft Project.",
                tutorial: "arquivo",
                procedure: 0,
                windows_only: false,
            },
            card(Icon::Save, "Salvar sem perder alterações", "Salvar, Desfazer, Refazer e o aviso ao fechar.", "arquivo", 1),
        ],
    ),
    (
        "Exibição",
        &[
            card(Icon::ZoomIn, "Zoom e escala de tempo", "Aproxime ou caiba o projeto inteiro no Gantt.", "exibir", 0),
            card(Icon::Column, "Colunas e linhas", "Ajuste larguras e alturas da grade.", "grade", 2),
            card(Icon::Text, "Fonte", "Escolha a fonte e o tamanho do texto.", "tarefa", 0),
            card(Icon::Format, "Caminho crítico e folga", "Destaque as tarefas que definem o término.", "formato", 0),
        ],
    ),
    ("Assistentes", &[card(Icon::Help, "Etapix no Claude e no Codex", "Peça cronogramas em texto a um assistente.", "assistentes", 0)]),
];

/// The starting sequence at the top of the Central de Ajuda.
pub const START: &[(&str, &str, usize)] = &[
    ("1. Preparar: Projeto > Informações do Projeto (data de início)", "projeto", 0),
    ("2. Primeira operação: digitar tarefas, durações e predecessoras", "grade", 0),
    ("3. Acompanhar: % concluída, linha de base e Em Andamento", "tarefa", 2),
    ("4. Consultar: caminho crítico em Formato > Tarefas Críticas", "formato", 0),
];
