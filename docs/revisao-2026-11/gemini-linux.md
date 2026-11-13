# Revisão Somente Leitura — Suporte e Empacotamento Linux (Gemini)

**Data:** 2026-11-13
**Escopo:** Revisão estática de `app/src/fonts.rs`, `packaging/linux/*`, `.github/workflows/linux.yml`, `app/src/mpp.rs` e todas as ocorrências de `cfg(windows)` / `cfg(not(windows))` em `app/` e `setup/`.

---

## 1. Achados por componente

### 1.1 Empacotamento Linux (`packaging/linux/*`)

1. **Aviso incorreto sobre `.mpp` no pacote Linux**
   - **Gravidade:** Alta | **Classificação:** Certeza
   - **Local:** `packaging/linux/LEIA-ME.txt:11-13`
   - **Diagnóstico:** O texto afirma *"ARQUIVOS .MPP: No Linux não existe Microsoft Project, então arquivos .mpp não abrem. Use Project XML (.xml)"*. Isso contradiz `README.md:12` e `app/src/mpp.rs:3-11`, pois o Etapix lê, cria e salva `.mpp` nativamente no Linux.
   - **Proposta:** Substituir as linhas 11–13 por: *"ARQUIVOS .MPP E .XML: O Etapix abre, cria e salva arquivos .mpp e Project XML (.xml) diretamente no Linux, sem precisar do Microsoft Project."*

2. **`etapix.desktop` sem MIME type para `.mpp`, sem `StartupWMClass` e sem atualização do cache de ícones**
   - **Gravidade:** Média | **Classificação:** Certeza
   - **Local:** `packaging/linux/instalar.sh:14-31` (`MimeType` na linha 26)
   - **Diagnóstico:** O atalho declara apenas `MimeType=application/xml;`, impedindo que gerenciadores de arquivos (GNOME Files, Dolphin) ofereçam o Etapix em *"Abrir com..."* para arquivos `.mpp`. Falta também `StartupWMClass=etapix` para associar a janela Wayland/X11 ao ícone na dock, e `gtk-update-icon-cache` após gravar em `~/.local/share/icons/hicolor/256x256/apps/etapix.png`.
   - **Proposta:** Em `packaging/linux/instalar.sh:26`, usar `MimeType=application/vnd.ms-project;application/x-mspublisher;application/xml;`, adicionar `StartupWMClass=etapix` e chamar `gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true`.

3. **`desinstalar.sh` preserva `~/.config/etapix` sem avisar**
   - **Gravidade:** Baixa | **Classificação:** Certeza
   - **Local:** `packaging/linux/desinstalar.sh:1-9` e `app/src/fonts.rs:102-105`
   - **Diagnóstico:** O script remove binários, ícone e `.desktop`, mas mantém `~/.config/etapix/settings.txt`. Comportamento seguro para preservar preferências em reinstalações, mas não informado ao usuário.
   - **Proposta:** Informar no `echo` final de `desinstalar.sh:9` que preferências em `~/.config/etapix` foram mantidas (ou remover se passado `--purge`).

---

### 1.2 Pipeline CI Linux (`.github/workflows/linux.yml`) e Workspace (`setup/`)

4. **`cargo test --workspace` quebra no Linux por causa do crate `setup`**
   - **Gravidade:** Alta | **Classificação:** Certeza
   - **Local:** `Cargo.toml:13`, `setup/Cargo.toml:15`, `setup/src/main.rs:18-33, 99, 113`, `.github/workflows/linux.yml:28`
   - **Diagnóstico:** `setup` é membro incondicional do workspace (`Cargo.toml:13`), mas importa `std::os::windows::process::CommandExt` sem `#[cfg(windows)]` (`setup/src/main.rs:99`), embute `../target/release/etapix.exe` (`setup/src/main.rs:25-33`) e compila `windows_install` sem gate (`setup/src/main.rs:19, 113`). Por isso o CI Linux precisa rodar `cargo test -p pjsched -p etapix` (`.github/workflows/linux.yml:28`) em vez de `--workspace`.
   - **Proposta:** Proteger o conteúdo de `setup/src/main.rs` com `#[cfg(windows)]` e prover um `#[cfg(not(windows))] fn main() {}` vazio (além de mover a dependência `windows` para `[target.'cfg(windows)'.dependencies]` em `setup/Cargo.toml:15`), permitindo `cargo test --workspace` e `cargo clippy --workspace` no Linux.

5. **Workflow Linux não roda em `pull_request`, instala `clippy` sem executar e publica em toda push na `master`**
   - **Gravidade:** Média | **Classificação:** Certeza
   - **Local:** `.github/workflows/linux.yml:5-8, 23, 55-60`
   - **Diagnóstico:**
     1. O gatilho (`linux.yml:5-8`) tem apenas `push: branches: [master]` e `workflow_dispatch`; PRs que quebrem `#[cfg(not(windows))]` só falham após o merge.
     2. `linux.yml:23` instala `--component clippy`, mas nenhum passo executa `cargo clippy`.
     3. `linux.yml:55-60` executa `gh release upload "$ tag" Etapix-Linux-x86_64.tar.gz --clobber` em todo push na `master` (se não houver `pull_request` separado), sobrescrevendo o asset da última release.
   - **Proposta:** Adicionar `pull_request:` aos gatilhos, rodar `cargo clippy -p pjsched -p etapix --all-targets -- -D warnings` após os testes e condicionar os passos `Pack` / `Attach to the latest release` a `if: github.event_name != 'pull_request'`.

---

### 1.3 Fontes no Linux (`app/src/fonts.rs` e `app/src/main.rs`)

6. **Parsing frágil da saída do `fc-list` e ausência de cache/testes no Linux**
   - **Gravidade:** Média | **Classificação:** Certeza
   - **Local:** `app/src/fonts.rs:21-45, 74-90, 144-154`
   - **Diagnóstico:**
     1. `entries()` (`fonts.rs:23-29`) chama `fc-list : family style file` e divide cada linha com `line.splitn(3, ':')`. Se o caminho do arquivo contiver `:` ou se o `fc-list` emitir estilos localizados (ex.: `style=Regular,Normal` ou `style=Bold,Negrito` em `LC_ALL=pt_BR.UTF-8`), `style.split(',').next()` (`fonts.rs:37`) pode pegar o nome traduzido que não bate com `STYLES = ["Regular", "Book", "Roman", "Bold"]` (`fonts.rs:15-18`), descartando a fonte ou deixando `bold: None`.
     2. `#[cfg(all(test, windows))]` (`fonts.rs:144`) exclui o Linux dos testes de `fonts::installed()` e `settings_path()`.
   - **Proposta:** Usar `fc-list --format="%{file}\t%{family}\t%{style}\n"` separado por `\t`, verificar se qualquer item de `style.split(',')` pertence a `STYLES`, e remover `, windows` do `#[cfg(all(test, windows))]` em `fonts.rs:144` (ajustando o limiar ou aceitando ambiente sem fontes apenas se `fc-list` não existir).

7. **Caminho fixo da `DejaVuSans.ttf` em `setup_style` falha fora de Debian/Ubuntu**
   - **Gravidade:** Baixa | **Classificação:** Certeza
   - **Local:** `app/src/main.rs:73-77`
   - **Diagnóstico:** Quando nenhuma fonte do usuário está selecionada, o fallback Linux tenta apenas `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`. Em Fedora/RHEL (`/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf`) ou Arch (`/usr/share/fonts/TTF/DejaVuSans.ttf`), o arquivo não é achado e o app cai na fonte bitmap padrão do egui.
   - **Proposta:** Testar uma lista curta de caminhos conhecidos no Linux ou buscar `"DejaVu Sans"` / `"Liberation Sans"` em `fonts::installed()`.

---

### 1.4 Leitura/Escrita `.mpp` e demais `cfg(windows)` / `cfg(not(windows))`

8. **Mensagem de erro de `rename` cita Project aberto e testes nativos de `.mpp` restritos ao Windows**
   - **Gravidade:** Baixa | **Classificação:** Certeza
   - **Local:** `app/src/mpp.rs:221, 241, 271, 308`
   - **Diagnóstico:**
     1. Em `write_native` (`mpp.rs:221`), se `std::fs::rename` falhar (ex.: `/tmp` em filesystem diferente do destino ou permissão), a mensagem inclui `"(o arquivo está aberto no Project?)"` mesmo no Linux.
     2. O módulo `tests` (`mpp.rs:241`) está sob `#[cfg(all(test, windows))]`, embora `saves_without_project` (`mpp.rs:271`) e `whole_session_saves_without_project` (`mpp.rs:308`) testem justamente o caminho 100% nativo sem Microsoft Project.
   - **Proposta:** Habilitar `saves_without_project` e `whole_session_saves_without_project` em `#[cfg(test)]` multiplataforma (mantendo só `missing_project_gives_a_clear_message` específico por plataforma) e omitir o sufixo do Project no Linux em `mpp.rs:221`.

9. **Demais `cfg(windows)` / `cfg(not(windows))` verificados e corretos**
   - **Classificação:** Certeza (sem ação necessária)
   - **Locais:** `app/src/fonts.rs:95-105` (`XDG_CONFIG_HOME` / `~/.config/etapix/settings.txt`), `app/src/help/mod.rs:305-307` (oculta cartões `windows_only` e adapta texto do `.mpp`), `app/src/bin/etapix-mcp/integrations.rs:10, 113, 129` (registra MCP em `~/.claude.json` e `~/.codex/config.toml`), e `app/icon_build.rs:57` (executa `windres` só quando `CARGO_CFG_TARGET_OS == "windows"`).

---

## 2. Proposta concreta: Nomes de fontes Microsoft mapeados no Linux (`app/src/fonts.rs`)

### Objetivo
Exibir no seletor de fontes do Linux os nomes familiares **`Times New Roman`**, **`Calibri`**, **`Arial`** e **`Courier New`** mapeados respectivamente para **`Liberation Serif`**, **`Carlito`**, **`Liberation Sans`** e **`Liberation Mono`** quando estas estiverem instaladas no sistema, sem embutir fontes proprietárias da Microsoft.

### Onde e como alterar em `app/src/fonts.rs`

1. **Adicionar a tabela de equivalência métrica** logo após `STYLES` (`app/src/fonts.rs:18`):
   ```rust
   #[cfg(not(windows))]
   const LINUX_MS_ALIASES: [(&str, &str); 4] = [
       ("Times New Roman", "Liberation Serif"),
       ("Calibri", "Carlito"),
       ("Arial", "Liberation Sans"),
       ("Courier New", "Liberation Mono"),
   ];
   ```
2. **Injetar os apelidos em `installed()`** (`app/src/fonts.rs:86`, imediatamente antes de `families.sort_by_key(|f| f.name.to_lowercase());` na linha 87):
   ```rust
   #[cfg(not(windows))]
   for (ms_name, metric_equivalent) in LINUX_MS_ALIASES {
       let already_present = families
           .iter()
           .any(|f| f.name.eq_ignore_ascii_case(ms_name));
       if !already_present {
           if let Some(target) = families
               .iter()
               .find(|f| f.name.eq_ignore_ascii_case(metric_equivalent))
               .cloned()
           {
               families.push(Family {
                   name: ms_name.to_string(),
                   regular: target.regular,
                   bold: target.bold,
               });
           }
       }
   }
   ```
3. **Impacto no restante do app:** Nenhum outro arquivo precisa mudar. `App::font_picker` (`app/src/main.rs:1330-1363`) lista `self.installed_fonts()` e grava `self.font_name` em `settings.txt`; `App::apply_font` (`app/src/main.rs:587-595`) localiza a `Family` pelo nome e passa os caminhos `regular`/`bold` da fonte equivalente instalada para `setup_style` (`app/src/main.rs:78-89`). Além disso, recomendamos incluir `fonts-liberation` e `fonts-crosextra-carlito` como sugestão em `packaging/linux/LEIA-ME.txt`.
