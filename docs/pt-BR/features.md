# Para que serve o devpit

Agente é fácil de começar e difícil de acompanhar. Cinco terminais depois, você
já não sabe o que está rodando, quanto custou, nem qual deles você estava lendo.

O devpit existe para manter **cada etapa do trabalho visível e sob a sua mão** —
um terminal que você de fato está olhando, um quadro que diz onde cada frente
está, e um número em cada chamada de agente.

## O que ele faz

- **Um terminal alvo por projeto.** Trocar de frente troca o que está atrelado a
  ele. A sessão anterior continua rodando em segundo plano; só para de ocupar a
  tela.
- **Sessões que sobrevivem à janela.** Feche o devpit, abra de novo, e o que
  estava rodando continua rodando.
- **Uma worktree do git por frente de trabalho**, criada quando uma etapa
  precisa de checkout e removida quando você mandar — nunca com trabalho não
  commitado dentro.
- **Um quadro por projeto.** Mover um card executa trabalho de verdade — no
  terminal que você já tem aberto, não em um novo.
- **As colunas são suas.** Renomeie, reordene, crie as suas, decida qual executa
  o quê.
- **Cada coluna compõe a própria etapa** — qual agente, em qual modelo, com
  que fatia do contexto do card.
- **Teto de gasto em toda chamada de agente**, e o custo real escrito no card
  quando ela termina.
- **Agentes são arquivos** — markdown com frontmatter em `~/.devpit/agents/`, e
  os que a sua ferramenta já instalou. Nenhum vem com o devpit: os que você vê
  são os da sua máquina. Nada para recompilar, nada para registrar.
- **Um pane de navegador**, para a página que o projeto está servindo. Abre
  `localhost` em http, que é o que um servidor de dev responde, mantém a
  sessão de cada pane separada, e traz uma sessão já logada do Chrome, do
  Firefox ou do Safari — por navegador e por domínio, nunca sozinho. Deixar um
  agente dirigir a página que você der a ele ainda não está pronto.
- **Árvore de arquivos e diffs** ao lado do terminal, para revisar o que o
  agente fez sem precisar sair.
- **Um quadro Online.** O quadro, as colunas, os cards (título, descrição e
  plano), comentários, critérios de aceite, vínculos, histórico com autor e
  data e anexos ficam no devpit.app, onde o servidor consegue lê-los. O app
  chega a ele pela API de quadros e pelo MCP. Os anexos têm limites por arquivo,
  por conta e em todo o serviço.
- **O sync pessoal continua selado.** Preferências e configurações de projeto
  são selados com uma chave que só as suas máquinas têm; o devpit.app nunca
  abre esses blobs (`docs/sync.md`).
- **Na sua máquina.** Um quadro Local em SQLite, projetos, conversas e histórico
  de terminal nunca saem deste computador. A conta é opcional.
