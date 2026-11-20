<p align="center">
  <img src="web/src/assets/brand/mark.png" alt="" width="128">
</p>

<h1 align="center">devpit</h1>

<p align="center">
  Um app nativo para controlar seus agentes de código.
</p>

<p align="center">
  <a href="https://github.com/jholhewres/devpit/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/jholhewres/devpit/ci.yml?branch=main&style=flat-square&label=ci" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/licen%C3%A7a-Apache%202.0-blue?style=flat-square" alt="Licença: Apache 2.0"></a>
  <img src="https://img.shields.io/badge/rust-1.93+-dea584?style=flat-square&logo=rust&logoColor=white" alt="Rust 1.93+">
  <img src="https://img.shields.io/badge/status-inicial-e2795b?style=flat-square" alt="Status: inicial">
</p>

<p align="center">
  <a href="README.md">English</a> &middot; <b>Português</b>
</p>

<p align="center">
  <img src="docs/assets/hero.webp" alt="O chat do orquestrador do devpit ao lado do quadro de um projeto, com sessões de agentes rodando nos terminais" width="100%">
</p>

---

## O que é

Um app desktop que mantém seus agentes de código à vista: um terminal que você
está olhando, um quadro que diz onde cada frente está, e o custo de cada
chamada de agente.

## O que ele faz

- **Um terminal alvo por projeto**; as outras sessões seguem rodando em segundo plano.
- **Sessões que sobrevivem à janela**: são sessões do tmux.
- **Uma worktree do git por card**, criada quando uma etapa precisa.
- **Um quadro por projeto**, cujas colunas rodam etapas: um agente, uma sessão ou um comando seu.
- **Um teto de gasto em cada chamada de agente**, e o custo real no card.
- **Um orquestrador**: um chat que vê todos os projetos e entrega trabalho a sessões.
- **Um pane de navegador** para a página que o projeto serve.
- **Árvore de arquivos e diffs** ao lado do terminal.
- **Quadros locais ou online**: mantenha o trabalho nesta máquina ou leve um
  quadro pelo devpit.app; a conta é opcional.

## Onde ficam os dados

Um quadro Online guarda no devpit.app o quadro, as colunas, os cards (título,
descrição e plano), comentários, critérios de aceite, vínculos, histórico com
autor e data e anexos, que o servidor consegue ler. O app chega a ele pela API
de quadros e pelo MCP. Os anexos têm limites por arquivo, por conta e em todo o
serviço.

O sync pessoal é separado: preferências, configurações de projeto e arquivos
que você escolher continuam selados com uma chave que só as suas máquinas têm,
por isso o servidor nunca abre esses blobs. Um quadro Local em SQLite, projetos,
conversas e histórico de terminal nunca saem desta máquina.

## Instalar

```sh
curl -fsSL https://devpit.app/install.sh | sh
```

Linux e macOS; confere o download contra o `SHA256SUMS` da release. Windows,
download manual, atualização e desinstalação:
[docs/pt-BR/install.md](docs/pt-BR/install.md). Precisa de `tmux` e do
[Claude Code](https://claude.com/claude-code).

## Links

- [Documentação](https://devpit.app/docs)
- [O que ele faz, em detalhe](docs/pt-BR/features.md)
- [Como o quadro funciona](docs/pt-BR/board.md)
- [Compilando e rodando](docs/pt-BR/building.md)
- [O modelo](docs/the-model.md) · [Arquitetura](docs/architecture.md) (em inglês)
- [Changelog](CHANGELOG.md) · [Como contribuir](CONTRIBUTING.md)
- [Comunidade no Discord](https://discord.gg/yW8VUyY63b)
- [Licença: Apache 2.0](LICENSE)
