//! The pt-BR words of Devflow's tools — the three that set it up and the
//! stages' own — what each says it does and what its parameters'
//! descriptions say, by name, so `devflow.rs` stays the English base. Names,
//! keys, types and `required` are the base's own; only the words are
//! Portuguese.

use crate::mcp::Said;

/// A tool's pt-BR words, by the name it is listed under; `None` for a tool
/// the catalog does not hold yet, which stays in the English of the base.
pub(crate) fn said(name: &str) -> Option<&'static Said> {
    Some(match name {
        "devpit_devflow_status" => &Said {
            description: "O Devflow neste projeto: o seu estado (sem arquivo, desativado, ativo ou inválido, com a linha e o campo do erro), o que o devpit detectou, com o arquivo de que veio cada fato (comandos de teste, E2E e lint, Playwright ou Cypress, as CLIs de agente instaladas e autenticadas, uma chave do Decisions, o glossário e os ADRs), as verificações que recomenda, as decisões que a detecção deixou em aberto, o .devpit/devflow/devflow.yaml atual e `next`, o passo que vem a seguir. Leia primeiro.",
            params: &[],
        },
        "devpit_devflow_setup" => &Said {
            description: "Configurar o Devflow aqui. preset recommended (o padrão): briefing e plano aprovados pela pessoa, as verificações detectadas que podem rodar aqui como scripts (uma cujo programa falta, ou um alvo do make cuja receita chama um, fica sob `unavailable` com o motivo), um revisor devflow-security, o gate devflow-done, Decisions em sombra quando existe uma chave; custom: só o básico. `set` incorpora respostas por caminho pontuado. Uma prévia do YAML e dos seus arquivos, a não ser que apply: true, enviada só depois do sim da pessoa, que escreve somente a configuração pré-visualizada por último: uma que mudou desde então volta como nova prévia com applied: false. Nunca commita, e o Devflow fica desligado até a pessoa clicar na confirmação do devpit.",
            params: &[
                ("preset", "Onde a configuração começa. Padrão: recommended."),
                ("set", "Caminho pontuado para valor, sobre o preset, p.ex. { \"stages.execution.checks\": [{ \"name\": \"unit\", \"run\": \"make test\", \"report\": \"junit\" }] }."),
                ("apply", "Escrever os arquivos. Só depois de um sim explícito à prévia."),
            ],
        },
        "devpit_devflow_set" => &Said {
            description: "Mudar um campo do devflow.yaml deste projeto pelo seu caminho pontuado (review.block_on, review.reviewers, stages.execution.checks, stages.plan.approve): uma prévia com o valor antes e depois, a não ser que apply seja true, que o escreve só depois do sim da pessoa. Funciona num arquivo inválido, para consertar o campo que o erro nomeia. enabled: true e decisions.mode: enforce nunca são escritos aqui: são o clique da pessoa numa confirmação que o devpit mostra no app.",
            params: &[
                ("path", "O campo, p.ex. review.block_on ou review.reviewers.1 (um além do fim acrescenta)."),
                ("value", "O novo valor: string, número, true ou false, lista ou objeto, como o campo aceita."),
                ("apply", "Escrevê-lo. Só depois de um sim explícito à prévia."),
            ],
        },
        "devpit_devflow_skill" => &Said {
            description: "A skill do Devflow a seguir para uma etapa, como este projeto a resolve: a sua própria .devpit/devflow/skills/<nome>/SKILL.md, senão a da pessoa, senão a embutida do devpit. Chame-a antes de cada etapa e siga o que ela devolve; devflow-setup é para mudar a configuração. A skill de qualquer etapa é dada quando pedida, para rodar aquela etapa sob demanda: também uma que o devflow.yaml desliga, ou sem devflow.yaml. Uma parte que a skill diz carregar só quando pedida vem com `part`.",
            params: &[
                ("name", "A skill: devflow-<stage> ou devflow-setup."),
                ("part", "Uma parte da skill que carrega só quando pedida, como a skill a nomeia (verify, without-card, retro, publish, by-hand de devflow-review). Deixe de fora para a própria skill."),
            ],
        },
        "devpit_devflow_brief" => &Said {
            description: "Devflow: escrever um brief no quadro, como o card épico intitulado '[devflow] Brief: <título>' — objetivo, para quem, sucesso, fora do escopo, restrições e decisões. Só depois do sim explícito da pessoa ao seu restate. Sem `card` cria o épico; com `card` escreve naquele card, renomeando-o quando `title` é dado. `context` (o resumo da etapa context, uma seção markdown ## Context) substitui o contexto do épico. Campos deixados de fora ficam como estavam. Responde o id do card.",
            params: &[
                ("card", "Um card existente onde escrever o brief: o épico, ou o card de que o trabalho partiu."),
                ("title", "O título curto; o card fica '[devflow] Brief: <título>'. Necessário para um épico novo."),
                ("success", "Uma verificação observável: um comando, uma tela, um número."),
                ("context", "O resumo da etapa context, começando com ## Context."),
            ],
        },
        "devpit_devflow_plan" => &Said {
            description: "Devflow: o plano de um épico como os seus cards filhos, cada um intitulado '[devflow] <título>', o corpo como build, os critérios como checkboxes sob ## Acceptance criteria, os arquivos prováveis e o comando verify, e o seu depends_on como arestas de bloqueio que o quadro mostra: uma chave do plano, ou um card já neste quadro (de outro épico) pelo seu id ou #42. Sem apply ele só pré-visualiza e nada cria: o plano espera no épico, onde a pessoa pode movê-lo, cortá-lo ou editá-lo e criar os cards com um clique. apply: true cria os cards, só depois do sim explícito da pessoa à prévia, e somente o plano que a pessoa viu por último: cards que difiram viram uma nova prévia. Quando a pessoa já aprovou no quadro, a resposta diz, com os ids dos cards. Um orquestrador planeja o épico de outro projeto com `project`, mostra a prévia à pessoa e aplica com `approval` citando o sim dela; apply sem `cards` aplica a prévia como está. Com os cards criados, `merge: { from, into }` diz que o trabalho de um card foi para outro — ele termina na evidência daquele card — e `cut` tira um card do plano, fechando-o sem evidência; um e outro liberam os cards que o esperavam. Sem apply ele pré-visualiza o que muda; apply: true só depois do sim explícito da pessoa. Sem `epic`, sob demanda onde o Devflow estiver ligado em Configurações: os cards lidos e verificados como prévia, criados em nenhum lugar e guardados em nenhum.",
            params: &[
                ("epic", "O id do card épico. Deixe de fora para só uma prévia."),
                ("key", "Um id local curto, p.ex. s1, a que o depends_on se refere."),
                ("title", "O comportamento, nas palavras do glossário; a tool acrescenta '[devflow] '."),
                ("build", "O que funciona de ponta a ponta quando chega, um parágrafo."),
                ("criteria", "Condições testáveis; viram checkboxes."),
                ("files", "Os arquivos que provavelmente toca."),
                ("verify", "Um comando que prova os critérios."),
                ("depends_on", "As chaves dos cards que o bloqueiam, ou um card já no quadro pelo seu id ou #42."),
                ("apply", "Criar os cards. Só depois do sim explícito da pessoa à prévia."),
                ("approval", "Somente o orquestrador, com apply: o sim que a pessoa lhe deu, passado adiante."),
                ("quote", "A mensagem da pessoa dizendo sim, nas palavras dela."),
                ("project", "Somente o orquestrador: o projeto do épico, por id ou nome."),
                ("merge", "Em vez de epic e cards: um card criado cujo trabalho foi para outro."),
                ("from", "O card mergeado, por id ou #número."),
                ("into", "O card que agora guarda o seu trabalho."),
                ("cut", "Em vez de epic e cards: um card criado tirado do plano, por id ou #número."),
            ],
        },
        "devpit_devflow_evidence" => &Said {
            description: "Devflow: rodar as verificações de um card do .devpit/devflow/devflow.yaml no checkout em que você está, cada uma pelo step runner do devpit com o card no seu ambiente; guardar os relatórios, screenshots e vídeo na pasta do próprio devpit para o projeto, nunca no repositório (as 5 execuções mais novas por card; o `run` da resposta diz onde); anexá-los ao card como artefatos, com um comentário ligando cada um; e responder com um resumo curto: cada verificação passou, falhou, ficou inconclusiva ou não rodou (uma verificação configurada para escrever relatório que não deixou nenhum é lida pelo devpit como inconclusiva; uma com `report: none` é julgada pelo código de saída, 127 significando que o seu comando não foi achado; uma cuja sonda `requires` falhou não roda, e verificações que nomeiam o mesmo lock `exclusive` nunca se sobrepõem), as suas contagens, as falhas e os caminhos. Ela registra os testes de cada critério e como terminaram, e os testes vermelhos que você viu e se estão verdes agora. Uma execução verde no mesmo commit e arquivos rastreados é reaproveitada, as suas verificações não rodam de novo (o `reused_from` da resposta a nomeia), a não ser com `fresh`. Deixe `checks` de fora para rodar todas. Demora o que as verificações demorarem.",
            params: &[
                ("card", CARD_ID),
                ("checks", "Verificações pelo seu nome no devflow.yaml. Deixe de fora para rodar todas."),
                ("criterion", "O texto do checkbox, literal."),
                ("tests", "Os testes que o provam, como o runner os nomeia, p.ex. tests/test_cli.py::test_rename_changes_the_title."),
                ("proof", "Um arquivo que o mostra onde nenhum teste pode, p.ex. um screenshot de mudança de UI: o seu caminho na pasta da execução ou no checkout. Um caminho que não está lá não conta."),
                ("failure", "A linha da falha que você viu, segredos removidos."),
                ("fresh", "Rodar as verificações mesmo quando a execução mais nova estava verde no mesmo commit e arquivos rastreados, cujos resultados seriam reaproveitados."),
            ],
        },
        "devpit_devflow_review" => &Said {
            description: "Devflow: revisar a mudança de um card com o review.reviewers do devflow.yaml, cada um headless na própria CLI de agente da pessoa (um ausente ou desconectado é pulado), a partir de um packet: o diff da base do card até o HEAD (do HEAD de uma execução anterior com `previous`), o card, os seus critérios e a persona. Escreve findings.json para o gate e comenta a contagem no card. Responde o veredito por review.block_on, o tempo e o custo de cada revisor, e `pending`: personas que nenhum revisor cobriu — revise-as você mesmo e chame de novo com `run` e `reports`. Com review.verify ligado, `next` nomeia os criticals e highs cujo PoC rodar: envie o que cada um mostrou com `run` e `verified`. Sem `card`, sob demanda: um pull request do GitHub (`pr`: BASE é o seu merge-base com o branch base do PR, HEAD o seu commit de cabeça, os checkboxes do corpo, senão o título e o corpo, os critérios), um intervalo (`base` e `head`) ou uma worktree do projeto (`path`), lidos numa worktree destacada ou na nomeada, para o checkout da pessoa nunca se mover; sem devflow.yaml os revisores são os da própria pessoa. Essa execução fica sob reviews/ do projeto na home do devpit e é anexada, e nenhum card é comentado. Um intervalo já mergeado e em produção é revisado com mode: retro: o seu veredito é follow_up, nunca request_changes, e cada defeito medium ou pior carrega `impact`, a consulta só de leitura que mostra o que ele tocou desde o deploy. Demora o que os revisores demorarem.",
            params: &[
                ("card", "O id do card. Deixe de fora para revisar `pr`, `base` e `head`, ou `path`."),
                ("base", "O branch ou commit de onde o diff começa, quando não é a base do próprio card ou o branch padrão de uma worktree. Normalmente deixado de fora com um card; com `head`, onde o intervalo começa."),
                ("head", "Sem card, com `base`: o branch ou commit onde o intervalo termina."),
                ("pr", "Sem card: um pull request do repositório GitHub do projeto, por número ou URL."),
                ("path", "Sem card: uma worktree deste projeto, revisada no seu próprio HEAD."),
                ("criteria", "Sem card: a que a mudança se compromete, uma linha cada."),
                ("mode", "Sem card, com `base` e `head`: o intervalo já está mergeado e em produção, então é acompanhado, nunca segurado."),
                ("previous", "Depois de corrigir os achados bloqueantes de uma revisão: a execução daquela revisão. Os revisores leem só o que mudou desde o HEAD dela, com os seus achados bloqueantes para conferir de novo; os outros achados dela são levados. Deixe de fora `base` e `run`."),
                ("run", "A execução que uma primeira chamada respondeu, que `reports` completa."),
                ("reports", "Com `run`: um objeto de findings para cada persona em `pending` que você revisou, como descreve o devflow-findings.schema.json ao lado do packet da execução."),
                ("verified", "Com `run` e sem `reports`: o PoC que você rodou para cada critical e high, como o review.verify pede."),
                ("finding", "O lugar do achado no `findings` do findings.json da execução, a partir de 0."),
                ("evidence", "Onde o PoC rodou e o que mostrou, ou por que não rodou."),
            ],
        },
        "devpit_devflow_publish" => &Said {
            description: "Devflow: uma revisão sem card, rascunhada para o seu pull request, como publish.json ao lado do seu findings.json: inline, cada critical, high e medium numa linha que o diff mostra, um comentário curto por linha, quaisquer que sejam os seus eixos; num comentário geral, os low e info como nits numerados, o que está fora do diff (um critical ou high aí como um ticket à parte), a verificação e o que está bom, no template do destino (um reviewers/publish/<destino>.md de uma camada substitui o do devpit). Sem apply ele rascunha e nada posta: mostre o rascunho à pessoa. apply: true, só depois do sim explícito dela e somente ao rascunho que ela viu por último (um que mudou desde então volta como o novo rascunho), o posta no pull request do GitHub com gh: uma revisão que só comenta, a não ser que a pessoa tenha pedido approve ou request changes, e o comentário geral. Publicado de novo, edita esses comentários em vez de postar novos. O rascunho de uma revisão retro também carrega um ticket de follow-up por achado medium ou pior, cada defeito com a sua consulta de impact, e nunca pede mudanças. gitlab e bitbucket são rascunhados para colar.",
            params: &[
                ("review", "A pasta e a execução da revisão como devpit_devflow_review as respondeu, p.ex. pr-42/20261016T120000Z."),
                ("destination", "Para onde vai. Padrão: github."),
                ("event", "O que a revisão pede. comment, a não ser que a pessoa tenha pedido mais."),
                ("apply", "Postar o rascunho. Só depois do sim explícito da pessoa a ele."),
            ],
        },
        "devpit_devflow_gate" => &Said {
            description: "Devflow: se um card pode ser chamado de done — a sua evidência mais nova passou depois da última mudança de código, cada critério mapeia para testes passando ou um arquivo de prova que está lá, o seu spec de E2E guardou um screenshot, findings.json não tem nada em review.block_on, cada etapa com gate passou; então, com o Decisions, a rúbrica devflow-done, que segura o card só em enforce. Um card de evidência ou doc (`kind`, ou uma linha `Kind: evidence` no corpo) que não mudou código fecha nas suas provas, sem revisão. Comenta no card; responde passed ou failed com o que falta e a etapa que o conserta. Chame-o com `card` antes de devpit_finish_card, que é recusado até ele passar.",
            params: &[
                ("card", CARD_ID),
                ("kind", "O que o card entrega, quando o corpo não diz. Normalmente deixado de fora."),
            ],
        },
        "devpit_devflow_stage" => &Said {
            description: "Devflow: uma das etapas do próprio projeto, um `uses:` no devflow.yaml além das embutidas (p.ex. acme/ticket, project/pr). Sem `card`: a sua skill e o seu gate, para seguir. Com `card`: roda o seu gate para o card (um comando pelo step runner do devpit no seu checkout, ou uma rúbrica do Decisions), guarda stage-<nome>.json na execução do card e comenta. devpit_devflow_gate exige que cada etapa com gate passe depois da última mudança. Um comando de camada de time roda só depois de a pessoa confirmá-lo no app.",
            params: &[
                ("stage", "O nome da etapa: o que segue o último / do seu uses."),
                ("card", "O card para o qual passar o gate da etapa. Deixe de fora para ler a skill da etapa."),
            ],
        },
        _ => return None,
    })
}

/// "The card's id."
const CARD_ID: &str = "O id do card.";
