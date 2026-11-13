# Revisão do instalador e alerta Defender — 2026-10-26

## Correções implementadas
- Atalhos via IShellLinkW/IPersistFile; caminhos são dados UTF-16, sem interpolação PowerShell.
- Registro de instalação HKCU via API Windows, sem reg.exe.
- Desinstalação sem CMD, ping ou processo destacado; remove somente os arquivos explicitamente conhecidos. Arquivos desconhecidos são preservados. O próprio desinstalar.exe permanece após o encerramento e a mensagem informa essa limitação.
- Falhas ao remover o conector ou arquivos passam a ser reportadas.
- Build com -ForPublication exige assinatura Authenticode válida e carimbo de tempo em aplicativo, conector e instalador. Assinatura do conector ocorre antes de incluí-lo nos arquivos de integração; assinatura do instalador ocorre após incorporar os binários. Build sem esse parâmetro não substitui Etapix-Instalador.exe da raiz.
- Ferramenta tools/sign-artifacts.ps1 aceita thumbprint de certificado no repositório CurrentUser/My. Não cria certificado autoassinado nem manipula confiança da máquina.

## Validação
Dois testes aprovados: caminho UTF-16 com apóstrofo/Unicode e criação/leitura real de atalho COM em pasta temporária. Scripts PowerShell com sintaxe validada; gate de publicação recusa executável sem assinatura. Compilação e Clippy registrados na sessão.

Primeira tentativa de varredura Defender via MpCmdRun terminou com erro 0x80508023 e exit 2. Não é resultado limpo nem comprovação de detecção do candidato. Proteção em tempo real permaneceu ativa. Assinaturas observadas: 1.461.215.0, motor 1.1.26090.9. Nenhuma exclusão ou desativação aplicada.

## Pendências
- Assinar usando identidade real do editor e certificado confiável, ainda não fornecido.
- Concluir varredura do candidato e dos executáveis internos; submeter a amostra exata à Microsoft e registrar resposta.
- Validar instalação, atualização e desinstalação em Windows isolado; não executado contra configuração real do usuário.
- Não publicado, instalado ou enviado a serviços externos nesta revisão. O arquivo público antigo continua inalterado.
- Alterações não garantem ausência de alertas futuros nem estabelecem que a detecção original seja falso positivo.

## Pontos de melhoria
Migrar o instalador próprio para MSI padrão, eliminando a necessidade de reter o desinstalador em execução. Publicar versão imutável com hashes, assinatura e evidências da validação. Revisar dependências e ambiente de build antes de liberar.
## Resultado final da varredura
Candidato final: 22E55300114BF5F7B1DAAA6EC9932A5DB98697AA5EEFB84D581A91B519F07DCB. MpCmdRun, com caminho Windows nativo, retornou exit 0 e found no threats para o instalador final, aplicativo e conector. O erro inicial não se repetiu. Esse resultado é pontual e não garante ausência de alertas em outras máquinas ou assinaturas futuras. Build release e Clippy --all-targets -D warnings aprovados.

## Atualização autorizada do repositório
O usuário autorizou atualizar o GitHub após ser informado da assinatura e do teste completo pendentes. A cópia da raiz foi substituída pelo candidato examinado, com hash e limitações no README. Esta distribuição excepcional sem assinatura não altera o bloqueio do build -ForPublication, que continua exigindo assinatura. Edição dos assets da release depende de autenticação GitHub disponível.

## Reabertura da investigação após o teste de download
O usuário reproduziu a detecção. Registros Defender de 26/10/2026 às 16:53:05 e 16:58:39 apontam o download da URL master; o arquivo já não estava disponível para hash. A leitura HTTP atual do master e do commit imutável confirmou SHA256 22E55300114BF5F7B1DAAA6EC9932A5DB98697AA5EEFB84D581A91B519F07DCB. O digest do asset da release também coincide. Isso confirma a distribuição do candidato e invalida a conclusão de que o scan local bastaria para liberar; não comprova o hash da amostra colocada em quarentena. README e release sinalizados como investigação aberta. Não houve restauração de quarentena, exclusão ou mudança na proteção. Pendente análise da Microsoft da amostra exata e novo ensaio isolado antes de liberar.

## Migração MSI em investigação — 26/10/2026

PR: https://github.com/Pontesrrafael/etapix/pull/3

Implementado MSI declarativo WiX 4.0.6, por usuário, sem CustomAction, scripts ou execução automática. Nome e pasta separados do instalador EXE legado; não migra nem remove automaticamente o legado. O conector MCP é copiado de forma passiva e suas configurações atuais não são alteradas. Há UpgradeCode e GUIDs estáveis.

O build usa os executáveis release já compilados e gera hash; -ForPublication mantém exigência de assinatura com carimbo de tempo. A versão do pacote MSI é independente da versão do motor. O texto de versão no LEIA-ME é um canário de upgrade; o teste confere também os estados dos ProductCodes, hashes instalados e preservação de arquivos desconhecidos e configurações após cada fase.

A primeira execução limpa no GitHub Actions (40402410774) aprovou testes Rust, build MSVC e geração MSI. O runner hospedado estava sem Defender ativo: a tentativa de varredura falhou e não comprova segurança. A verificação seguinte registra explicitamente NOT_TESTED nessa condição, permitindo testar o ciclo de instalação em ambiente descartável; build verde não significa antivírus aprovado. A aceitação de distribuição exige validação adicional em endpoint protegido e download real, além de assinatura/análise Microsoft pendentes.

Revisão Gemini 4 Argon em modo plan, sem ferramentas: ajudou a definir os critérios. Alegações de que MSI eliminaria a detecção, ou de que um alerta SmartScreen seria inevitável sem certificado, não foram aceitas como fatos. A segunda resposta parcial reforçou verificações de upgrade e separação de evidências. Revisão independente do código confirmou ausência de custom actions explícitas e apontou lacunas de teste já corrigidas.