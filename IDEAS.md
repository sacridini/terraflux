# Ideias para o Terraflux

Ideias levantadas no desenvolvimento que ficaram fora do escopo atual, para implementar em outro momento. O
plano e o andamento estão em [`docs/PLAN.md`](docs/PLAN.md). Uma ideia concluída é marcada como **Feito**
(versão).

## Próximas partes do landschaft no builder

Registro de 2026-10-11. O MSPA já está no builder (`landscape.mspa`, bit a bit com o GTB), com os modelos
neutros (`nlm.*`) para testar fluxos sem dados. A ordem sugerida:

1. **O resto do GuidosToolbox.** O C++ está em `extern/landschaft/src/cpp/gtb` e a biblioteca `gtbcore` já é
   linkada pelo `terraflux-ls-sys`: falta a ponte (shim + operador), como foi com o MSPA. São raster → raster e
   encaixam direto no builder.
   - **Feito** (Unreleased): FAD e FAD multiescala, FOS, SPA e Accounting, bit a bit com as saídas do GTB, com
     as paletas e os nomes das classes no viewer.
   - **Feito** (Unreleased): contagion e clustering, entropia espacial, landscape mosaic, gray spatial
     convolution, parcellation e contortion, distância euclidiana, mudança simples, morfológica e do FOS.
   - **Feito** (0.14.0): GTN (componentes, importância, Conefor), o Restoration Planner (status, resistência,
     custo, caminho de menor custo, corredor, isócronas), zonas de influência e proximidade, a mudança do FAD
     multiescala, o heatmap e o delta do landscape mosaic, e os relatórios de texto do FOS, SPA e Accounting no
     formato do GTB.
   - Cada produto com a sua paleta e os nomes das classes no viewer, como o MSPA e o FAD
     (`terraflux_ops::landscape::Product`; os do landschaft estão em `src/landschaft/_plotting/_palettes.py`:
     mudança simples e morfológica, a ordem da legenda do GTB).
2. **Métricas de paisagem** (`lsm`). **Feito** (Unreleased): por mancha, classe, paisagem e em janela móvel,
   iguais às do landscapemetrics do R; o visualizador de tabelas ordena, filtra e desenha uma coluna em barras.
   Falta do `lsm` do landschaft:
   - **Feito** (0.15.0): a análise espaço-temporal (`metrics.series`), a amostragem por pontos, buffers e
     polígonos (`metrics.sample`), as zonas em grade (`vector.grid_zones`) e a correlação entre métricas
     (`metrics.correlation`);
   - **Feito** (Unreleased): a análise de assinaturas (`metrics.signatures`, `metrics.pca`, `metrics.nmf`,
     `metrics.cluster`, `metrics.clustergram`), os mesmos grupos do landschaft, com o clustergram no viewer.
     Os grupos voltam às zonas por `vector.join` (e `vector.rasterize`, um mapa dos grupos);
   - **Feito** (Unreleased): paisagens fora da memória (`block_rows` nos nós de métricas, o `TiledLandscape`
     do C++), o mapa das manchas de um raster em disco gravado bloco a bloco, e o MSPA por tiles do GTB
     (`landscape.mspa_tiled`). **Feito** (Unreleased): as outras métricas por blocos (janela móvel por faixas com
     halo, amostras janela a janela, séries e assinaturas de rasters em disco);
   - **Feito** (Unreleased): gráficos de várias métricas de uma vez (barras, linhas e pontos por grupo, o
     clustergram, a matriz de correlação), os box plots por grupo e o círculo de correlações do PCA.
3. **Planejamento sistemático de conservação e restauração.** **Feito** (Unreleased): o prioritizr (tabelas,
   portfólios, contiguidade, limites manuais, filogenia, calibração e zonas de manejo), o prioriactions
   (`prioritize.actions`) e o planejador do plangea (`restoration.plan`: exato e em passos, conservação e
   conversão, curvas de trade-off, mudanças projetadas, redução do risco de extinção), iguais às referências do
   landschaft. Falta:
   - **Feito** (0.10.0): no planejador do plangea, o benefício de integridade (linearizado ou exato), o
     objetivo de custo mínimo e a comparação de pesos;
   - **Feito** (0.11.0): os extras do Zonation (condições, retenção, conectividade, impactos, mapas de
     pré-processamento e pós-processamento), os 65 casos do landschaft;
   - **Feito** (0.12.0): o risco de extinção como objetivo do prioritizr (`min_extinction_risk`);
   - **Feito** (0.13.0): unidades de planejamento em polígonos no prioritizr (com os vetores no builder).
   - **Feito** (Unreleased): polígonos com zonas de manejo (o `polygon_zone_units` do landschaft, bit a bit com o
     compile() do prioritizr) e as unidades H3 (`prioritize.h3_units`, pelo h3o; os mesmos hexágonos e custos).
   - **Feito** (0.69.0): o prioriactions de rasters (`prioritize.actions_raster`: as unidades nas células com
     custo de monitoramento, features e ameaças em camadas, custos das ações em números ou mapas, a solução em
     mapas), as mesmas tabelas e os mesmos ótimos do landschaft.
   - **Feito** (0.68.0): os projetos do Zonation 5 (`prioritize.zonation_project`: o `.z5`, a lista de features
     e os arquivos de ligação, o pós-processamento), os mesmos rank maps do z5 nos 65 projetos do landschaft. Falta
     escrever as saídas nos arquivos do z5 (`rankmap.tif`, as curvas, `features_info.csv`, pós-processamento e
     impactos), dígito a dígito como o landschaft (as somas do Eigen em float).

## Vetores

Os vetores entraram em 0.13.0 (leitura e escrita pelo OGR, rasterização, estatísticas zonais, somas exatas,
distâncias, unidades de planejamento em polígonos, o desenho no viewer).
- **Feito** (Unreleased): vetores sobre os mapas no viewer, e sozinhos, desenhados em tiles por threads a partir
  de um R-tree (camadas de milhões de feições sem travar), coloridos por atributo; `vector.extract` (o do
  landschaft, bit a bit), `vector.buffer`, `vector.clip` e `vector.dissolve` (o GEOS pelo GDAL), `vector.join`.
- **Feito** (Unreleased): os vetores no motor por tiles: `vector.zonal` por faixas de linhas (as zonas grandes
  corrida por corrida, as mesmas somas do C++), `vector.extract` pelas faixas que têm pontos e `vector.rasterize`
  num GeoTIFF faixa a faixa, bit a bit com a memória (exceto as linhas do GDAL com `all_touched` nos cantos das
  células). **Feito** (0.63.0): `vector.exact_sum` e `vector.distance` fora da memória, os mesmos bits.

## Viewer

- O viewer escolhe as cores pelo tipo do dado (classes, rampa, divergente em torno de 0, terreno, hillshade,
  NDVI, MSPA), pelo operador que fez o valor e pelos nomes de nós, arquivos e bandas.
- **Feito** (Unreleased): o produto gravado com o mapa (`landschaft_product` no GeoTIFF) e lido de volta; a
  escala logarítmica (correntes de circuito, e à mão); a rampa pelos percentis 2–98; a paleta trocada à mão no
  painel e lembrada por saída.
- O Viewer como central de mapas (camadas, comparação, vistas no lugar do `view.plot`): o plano em
  "O Viewer como central de mapas, e as vistas no lugar do `view.plot`".

## Builder

- **Feito** (Unreleased): reprojeção, vetores e distâncias com um clique: o painel Prepare compara o dado do nó
  com a grade de referência e oferece os passos (para a grade, reprojetar, declividade, distância à classe; dos
  vetores, distância, rasterizar, estatísticas zonais), cada um um nó com a nota do porquê.

- **Feito** (Unreleased): navegar pelos nós com o teclado (setas com o ponteiro sobre o canvas, Enter abre a
  saída); templates do Dinamica EGO e do CLUE-S (dados desenhados por modelos neutros); templates do próprio
  usuário (salvar o modelo como template, vê-lo e removê-lo no menu).

- **Feito** (0.80.0): templates para mostrar as possibilidades da ferramenta, em grupos no menu (padrões,
  métricas, conectividade, conservação e restauração, espécies, mudança de uso, superfícies e incerteza): mudança
  de uma floresta, manchas, tipos de paisagem, redes de habitat, circuitos, custos e caminhos aleatórios,
  corredor, avaliação de um plano, espécie num clima mais quente, fatores da mudança, autômato celular num laço,
  agentes, CRAFTY, análise e validação da mudança, terreno, carbono com incerteza, mais modelos neutros.

## O modelo como registro do que um agente fez: ler, conferir, decidir

Registro de 2026-10-18 (conversa na sessão do landschaft).

**A premissa.** Num mundo em que quase ninguém programa e muitas vezes nem monta o modelo à mão, quem monta é um
agente (pelo `terraflux mcp`). O modelo do Terraflux passa a ser o **registro da lógica** de um processamento,
sem depender de código em Python ou R. Para isso o formato atual já é o certo, e melhor que um script:

- tem vocabulário fechado e tipado: o agente só usa operadores que existem, e uma conexão errada é recusada na hora;
- cada nó tem uma nota, ou seja, a intenção fica junto da lógica;
- o arquivo é declarativo, versionado (`terraflux/1`) e comparável por diff;
- roda sem Python e sem R, e os resultados intermediários podem ser inspecionados;
- o `watch.rs` já segue as edições do agente, com ctrl+Z para desfazer.

O grafo continua sendo a **fonte única da verdade**. O que muda é a interface. Hoje ela foi feita para quem
**monta**, e o canvas de nós vira um emaranhado de fios para quem só precisa entender e auditar um modelo de 40 a
60 nós. Ela precisa servir a quem **lê, confere e decide**. Tudo abaixo são vistas novas sobre o mesmo grafo, sem
mudar o motor. Só a fase 1 acrescenta campos opcionais ao formato.

**Fase 0: protótipo antes de código.** **Feito** (Unreleased): os protótipos de três modelos e as decisões em
`docs/DOCUMENT_VIEW.md`. Desenhar à mão a vista documento (fase 2) de dois ou três modelos grandes
reais (um CLUE-S completo, uma priorização com zonas, um SWAT com calibração) para ver se ela dá conta: a ordem
das etapas, o que cabe num passo, onde a miniatura ajuda e onde atrapalha.

**Fase 1: o porquê, além do quê (formato).** **Feito** (1.0.0): os campos no nó e no modelo, as ferramentas
do MCP e do assistente (`set_model_info` nova), o aviso do `add_node` e do `check_model` (fora os nós copiados de
um template), o assistente assinando o que muda (autor e pedido), o registro no painel Node e o do modelo sem nó
selecionado.
- Campos opcionais no nó, ao lado da `note`:
  - `why`: a justificativa da escolha;
  - `refs`: artigos, métodos, a documentação do operador;
  - `assumptions`: as premissas;
  - `alternatives`: o que foi considerado e descartado, e por quê.
- No nível do modelo:
  - `request`: o pedido original do usuário, nas palavras dele;
  - `summary`: um resumo da conversa que levou ao modelo;
  - `author`: pessoa, agente ou os dois.
- As ferramentas MCP aceitam e preenchem esses campos.
- O `check_model` avisa (sem recusar) quando um nó feito pelo agente vem sem `note` ou sem `why`.
- O formato continua `terraflux/1`: são campos opcionais e os modelos antigos seguem válidos.

**Fase 2: a vista documento (a vista principal do usuário final).** **Feito** (Unreleased): o painel Document
(as cinco etapas, a ordem das dependências e do canvas, os passos com nota, porquê, entradas, parâmetros em
palavras, expressões ligadas aos nós e o registro; os dados de um tipo numa linha, o encanamento numa linha; o
corpo dos laços e grupos dentro do passo; os `view.plot` como figuras numeradas; os mapas e tabelas do último run;
selecionar e editar no lugar) e o relatório em HTML e PDF (o painel, File › Export report, `terraflux report`).
Falta: frases escritas à mão por
operador para os parâmetros, editar no lugar listas e tabelas (hoje pelo painel Node) e as cores das paletas do
viewer nas miniaturas.
- **O que ela mostra.** O mesmo grafo como uma seção de métodos de artigo, com as etapas em ordem topológica:
  dados → preparo → modelo → validação → resultados. Cada passo traz:
  - a nota e o porquê;
  - os parâmetros em linguagem natural, tirados da descrição do operador ("alcance de 30 células", e não
    `range = 30`);
  - as expressões do `raster.calc` com os nomes ligados aos nós de origem;
  - uma miniatura do resultado, tirada do cache do runner.
- **Edição.** Clicar num passo abre o nó. Os parâmetros podem ser mudados ali mesmo, e o modelo roda de novo como
  no builder.
- **Exportação** como relatório (HTML e PDF): o registro auditável de um processamento.
- O canvas vira a vista avançada, para quem quiser os fios.

**Fase 3: revisão das mudanças do agente.** **Feito** (1.0.0): o painel Changes, aberto pelas mudanças do
assistente e pelas do arquivo escritas por outro programa (o `terraflux mcp`); o diff semântico (nós, conexões,
configurações, registro, entradas e saídas, antes e depois), os mapas antes e agora lado a lado, os nós mudados
contornados no canvas, aceitar ou rejeitar por mudança ou tudo de uma vez; comparar com o arquivo salvo, outro
arquivo ou o último commit. Falta: comparar com um commit qualquer (hoje só o último) e as tabelas lado a lado
(hoje em palavras).
- Hoje, quando o agente edita, o modelo é recarregado e dá para desfazer, mas não dá para ver *o que mudou*.
  O `watch.rs` passa a abrir uma revisão como a de um pull request:
  - um diff semântico do grafo: nós adicionados, removidos e alterados, conexões refeitas, parâmetros com o antes e
    o depois;
  - os mapas e tabelas afetados antes e depois, lado a lado;
  - aceitar ou rejeitar por mudança, ou tudo de uma vez.
- A mesma vista serve para comparar duas versões quaisquer do arquivo, por exemplo dois commits.

**Fase 4: hierarquia e zoom semântico.** **Feito** (Unreleased): agrupar os nós selecionados (ctrl+G) e desfazer
um grupo (ctrl+shift+G); "Group the model by stage" (um grupo por etapa, cada um com a sua nota); os grupos como
blocos ligados pelo que os corpos leem, com as notas embaixo; com zoom afastado, as etapas emolduradas e nomeadas
num modelo sem grupos; uma seção do documento por grupo; o grupo guardando o que o corpo deu (as miniaturas dos
passos); `group_nodes`, `ungroup` e `group_by_stage` no MCP e no assistente, com a orientação de agrupar por
etapa. Falta: ver no viewer o mapa de um nó de dentro de um grupo (hoje pelo documento), o cache por nó dentro de
um grupo (hoje o grupo roda inteiro quando um nó muda).
- Etapas por `control.group`. As orientações do `terraflux mcp` pedem ao agente que sempre agrupe por etapa.
- O nível de cima do canvas mostra de 5 a 7 blocos, cada um com a sua nota, e abre um bloco por vez.
- Com zoom afastado, o canvas mostra só as etapas; com zoom aproximado, os nós.

**Fase 5: verificações visíveis.**
- Operadores `check.*` que não mudam os dados e dão um selo verde ou vermelho com a razão:
  - área conservada entre passos;
  - sem nulos inesperados;
  - valores dentro de uma faixa;
  - acurácia acima de um mínimo;
  - soma de demandas igual à alocada.
- O agente as escreve junto com o modelo.
- Os selos aparecem na vista documento e no canvas, e um resumo no topo mostra de relance se o modelo está
  saudável.

**Fase 6: parâmetros como painel.** **Feito** (Unreleased): `parameters` e `scenarios` no formato (opcionais); o
painel Parameters (deslizante numa faixa, escolhas, sim ou não, campo; cada mudança uma edição que roda o modelo
de novo); salvar os valores como cenário, aplicar um cenário, mandar os cenários ao Many runs e as faixas à sua
calibração, comparar cenários lado a lado (mapas, tabelas, números e as diferenças do primeiro); `terraflux run
--scenario`; as ferramentas `set_parameter`, `remove_parameter`, `set_scenario` e `remove_scenario`; o documento
e os relatórios com os parâmetros e cenários. Um parâmetro é uma entrada `$nome` ou uma configuração de nó
(`nó.configuração`), porque uma configuração ainda não lê uma entrada (decisão pendente 5). Falta: a comparação
pelos lotes distribuídos (hoje no app, um cenário depois do outro) e as diferenças em mapas de classes (hoje a
subtração dos valores).
- As entradas `$nome` do modelo ganham tipo, faixa, unidade e texto de ajuda (no formato, como os demais campos
  opcionais). Viram um formulário simples, com controles deslizantes e escolhas, para o usuário mudar um cenário
  sem tocar no grafo.
- O painel se liga aos cenários e à calibração do `batch_view`: salvar um conjunto de parâmetros como cenário e
  comparar cenários lado a lado (mapas, tabelas, diferenças).

**Fase 7: rastreio de uma célula.** **Feito** (Unreleased): o painel Trace (uma célula clicada num mapa, "Trace
this cell"): a cadeia de valores para trás, um cartão por nó com o valor ali e como foi feito (a expressão do
`raster.calc` com os valores no lugar dos nomes, a janela de células de um operador de vizinhança, as camadas que um
modelo pesa, um laço passo a passo, os grupos atravessados), as entradas clicáveis e uma frase no topo; "Trace
inside the loops" roda o modelo de novo guardando os valores do corpo dos laços na célula (o `Observer::body` do
motor). Com o assistente: "Ask the assistant why" manda o rastreio ao chat, e a ferramenta `trace_cell` (no app e
no `terraflux mcp`, com `inside_loops`) dá a mesma cadeia a qualquer agente. O rastreio é do app; a IA só explica
o que ele deu. Falta: a célula marcada nos mapas de cada passo ao selecionar um cartão, a mancha inteira de um
operador de manchas (hoje a janela ao redor) e a demanda do passo numa alocação explicada em palavras.
- Clicar num pixel do resultado mostra a cadeia de valores que o produziu, por exemplo: "esta célula virou cidade
  porque a adequação era 0,71, tinha 4 vizinhos urbanos e o limiar no passo 12 era 0,50".
- **Implementação.** Percorrer o grafo para trás a partir do nó mostrado e ler o valor da célula na saída em cache
  de cada nó de origem. Nos laços, usar os passos coletados (com uma opção de coletar todos ao rastrear). Para o
  `raster.calc`, mostrar a expressão com os valores substituídos. Operadores de vizinhança e globais (`nbsum`,
  manchas, alocação) mostram o valor e uma explicação curta do que entra nele (a janela, a mancha, a demanda).
- É a pergunta mais comum de quem audita um modelo espacial, e nenhuma ferramenta do gênero responde bem: é o que
  pode diferenciar o Terraflux.

**Fase 8: o agente dentro do app.** **Feito** (0.99.0): o painel Assistant, com as ferramentas do MCP no
processo (as edições no modelo aberto, cada uma desfeita com ctrl+Z, o nó tocado selecionado; o run no app), o
botão Explain do nó selecionado, o cliente da API compatível com a da OpenAI (streaming, tool calling, o
pensamento do Qwen), os servidores conhecidos, a chave no cofre do sistema, "testar conexão" e o conjunto reduzido
de ferramentas. Testado com o Qwen da HU Berlin (`llm3`, pela VPN). Faltam: destacar os nós durante a edição além
da seleção, a revisão da fase 3, a avaliação.
- Um painel de conversa ao lado do grafo, com as mesmas ferramentas do `terraflux mcp` chamadas dentro do
  processo.
- Clicar num nó e perguntar "por que isso?" ou "o que acontece se eu mudar este limiar?".
- O agente destaca os nós que está mexendo, e cada edição passa pela revisão da fase 3.
- O agente externo pelo MCP continua.
- **Qualquer provedor, inclusive o da universidade.** Quase ninguém na academia tem chave de API de um provedor
  comercial, mas muitas universidades hospedam modelos abertos (Qwen, Llama, Mistral). O chat é então um cliente da
  **API compatível com a da OpenAI** (`/v1/chat/completions` com *tool calling*), que o vLLM, o LiteLLM, o Ollama,
  o llama.cpp server e os serviços acadêmicos falam. Nas configurações:
  - o endereço (`base_url`);
  - a chave, quando o servidor pede;
  - o nome do modelo.

  O mesmo cliente serve para o servidor da universidade, para um Ollama no próprio computador (offline, sem
  chave), para a API do Claude e para outros provedores. A chave fica no cofre de credenciais do sistema, nunca no
  arquivo do modelo.
- **Servidores acadêmicos conhecidos** (a confirmar com cada centro de computação):
  - o KI-Service da HU Berlin, com modelos abertos locais e APIs para desenvolvimentos próprios (a página não diz
    quais modelos nem quem tem acesso);
  - o Chat AI da GWDG (Academic Cloud), com Qwen e outros modelos, API compatível com a da OpenAI para quem pedir;
  - o AI-ToolLab da Goethe-Universität, por um gateway LiteLLM.

  O que perguntar a cada um: se a API é compatível com a da OpenAI e aceita tool calling (no vLLM depende do
  `--tool-call-parser` do servidor), como se obtém um token (o chat web costuma usar SSO), e se a API só responde
  de dentro da rede ou pela VPN. Um botão "testar conexão" confere as três coisas e diz o que falhou.
- **Modelos menores.** Um Qwen de 30B é mais fraco que um modelo de fronteira em longas sequências de chamadas de
  ferramentas. O desenho ajuda:
  - o verificador recusa edições erradas com o motivo, e o modelo se corrige;
  - os templates são o ponto de partida, então o agente adapta em vez de criar do zero;
  - as descrições dos operadores vêm sob demanda (`describe_operator`), não o catálogo inteiro, para caber em
    janelas de contexto menores;
  - um conjunto reduzido de ferramentas para modelos pequenos (explicar, ajustar parâmetros, rodar, conferir);
  - a revisão da fase 3 garante que nada entra sem o usuário ver.

  Expectativa: explicar nós, ajustar parâmetros e montar fluxos pequenos e médios funciona bem; montar modelos
  grandes inteiros ainda pede um modelo de fronteira.
- **Avaliação.** Um conjunto fixo de tarefas (montar um template a partir da descrição, corrigir um modelo
  quebrado, explicar um resultado) roda contra cada modelo configurado e mede a taxa de sucesso. Isso mostra ao
  usuário o que esperar do modelo da sua universidade.

**Ordem e prioridade.** As fases 1 a 3 dão o maior ganho, porque fazem do arquivo um registro auditável e dão
confiança no que o agente faz. A 7 é o diferencial. A 4 e a 5 são baratas e podem entrar em paralelo. Tudo isso é
independente do port do landschaft para Rust (`RUST_PORT.md` no landschaft) e vale igual na versão web.

## A interface: o que falta nas partes já feitas

Registro de 2026-10-27. As sobras das vistas e painéis já lançados, juntadas de cada seção (as "Falta:" acima e
em "A calculadora de rasters" e "A linguagem do grafo"). Antes delas: a fase 5 da calculadora (o Assistant
escrevendo scripts). Depois, nesta ordem:

1. **Selos de verificação** (fase 5 de "O modelo como registro"): o `check.assert` existe, faltam os selos verde e
   vermelho com a razão no canvas e no documento, e o resumo da saúde do modelo no topo.
   **Feito** (Unreleased): `checks.rs`; o selo de cada `check.assert` (passou, só avisou, falhou e parou, não rodou
   ou desligado) no nó, com a razão no tooltip, e no seu passo do documento; um contêiner com o pior dos seus; a
   saúde na barra de status e no topo do documento. Falta: os selos no relatório (HTML e PDF) e os `check.*` como
   operadores próprios (hoje tudo é uma expressão do `check.assert`).
2. **Execução e depuração:**
   - **Feito** (Unreleased): o `manifest.json` também nas execuções do aplicativo (hoje só o `terraflux run`): uma
     execução inteira de um modelo salvo escreve as saídas e o `output/manifest.json` (`runner::Record`);
   - **Feito** (Unreleased): o Stop que para de verdade os operadores longos (MSPA, circuitos, ajuste dos SDM,
     `prioritize.evaluate`, métricas por blocos), com `ctx.progress(...)` entre as partes (telhas, lotes de pontos,
     dobras, lotes de soluções, orçamentos, blocos, faixas). Falta: dentro de uma chamada do C++ (o MSPA de uma
     telha, o `landscape.mspa` inteiro, o `circuit.pairwise`, o Omniscape) ainda não para; precisaria de um
     `watch` no landschaft, como o do `opt_solve_watched`;
   - **Feito** (Unreleased): congelar o resultado de um nó entre sessões (o cache em disco): `frozen = true` no nó,
     as saídas em `<modelo>.frozen/<nó>/` (`terraflux_exec::frozen`), seguradas no cache (`Cache::hold`) e usadas
     no lugar de rodá-lo, no aplicativo e no `terraflux run`; o que só ele lê não roda. Falta: congelar um nó dentro
     de um contêiner; os agentes, as árvores e os dados do Biodiverse em disco;
   - **Feito** (Unreleased): rodar um laço passo a passo, como um breakpoint (`runner::Gate`: Step, Next,
     Continue na barra de status). Falta: parar num nó escolhido (um breakpoint de verdade, não só entre passos).
3. **Vista Document:** frases escritas à mão por operador para os parâmetros; editar listas e tabelas no lugar
   (hoje pelo painel Node); as paletas do viewer nas miniaturas.
   **Feito** (Unreleased): `phrases.rs`, uma frase por operador (os 36 mais usados) com os parâmetros e as
   entradas, no documento e no relatório; as listas e tabelas editadas no lugar (o editor do painel Node,
   `node_view::values`); as miniaturas na paleta do viewer e na legenda do modelo (`builder::output_look`).
   Falta: frases para os outros operadores; as tabelas de mais de uma coluna (hoje só pares nome e valor).
4. **Painel Changes:** comparar com um commit qualquer (hoje só o último); as tabelas lado a lado (hoje em
   palavras).
   **Feito** (Unreleased): "Compare with a commit (git)" lista os últimos 50 commits do arquivo do modelo
   (`review::commits`) e compara com o escolhido; as tabelas de um nó antes e agora lado a lado
   (`review::tables_side_by_side`), as células que mudaram (por linha e nome de coluna) destacadas, as colunas
   novas ou removidas na cor delas. Falta: as linhas casadas por uma chave (hoje pela posição); a tabela inteira
   numa janela (hoje 12 linhas e 6 colunas).
5. **Grupos e etapas:** ver no viewer o mapa de um nó de dentro de um grupo (hoje pelo documento); o cache por nó
   dentro de um grupo (hoje o grupo roda inteiro quando um nó muda).
   **Feito** (Unreleased): dentro de um grupo, o nó selecionado mostra seu mapa no viewer, pelo que o grupo de
   cima dá (`inner.node.port`; `builder::shown_at`, `inner_of`), nomeado pelo lugar (`preparation/temperature`),
   com as suas saídas para escolher e o Trace a partir dele; o cache guarda as saídas dos nós do corpo de um grupo
   pelo lugar (`Cache.inner`, `group/node`), a impressão de cada um a do que entra no grupo mais a sua
   (`scoped_print`): ao mudar um nó, só ele e o que depende dele rodam de novo, nos grupos dentro de grupos também.
   Falta: o mesmo nos corpos dos laços e do switch (hoje rodam inteiros); congelar um nó de dentro de um grupo.
6. **Parameters e cenários:** a comparação pelos lotes distribuídos (hoje um cenário depois do outro, no app); as
   diferenças em mapas de classes (hoje a subtração dos valores).
   **Feito** (Unreleased): "Compare in Many runs" manda os cenários marcados como um lote do Many runs (o pool
   deste computador e dos que ajudam, ou um cluster) que guarda só a saída comparada; no fim, cada execução é lida
   de volta (`frozen::read_written`) e mostrada lado a lado no painel, na ordem dos cenários. Nos mapas de classes
   (como o viewer os mostra: a paleta do operador, a legenda do modelo, os valores), a diferença é a classe para
   a qual cada célula mudou (sem dado onde não mudou), com quantas células mudaram e as mudanças mais comuns
   (`class_changes`). Falta: a matriz de transições inteira; mais de um resultado comparado por vez.
7. **Trace:** a célula marcada nos mapas de cada passo ao selecionar um cartão; a mancha inteira de um operador de
   manchas (hoje a janela ao redor); a demanda do passo numa alocação explicada em palavras.
   **Feito** (Unreleased): o nome de um cartão seleciona o nó e mostra o seu mapa no viewer com a célula traçada
   contornada (dentro de grupos também); os operadores de manchas (`raster.label`, `raster.sieve`, MSPA,
   `metrics.patch`, `patches.observe`) dizem a mancha inteira da célula (`How::Patch`: a classe, as células unidas
   por 4 ou 8 vizinhos, como o operador junta, e as linhas e colunas que ocupa); uma alocação (`lulcc.step`,
   `dinamica.step`, sozinha ou num laço com o corpo guardado) diz em frases os passos em que a célula mudou de
   classe e a demanda de cada um (as células de cada classe e quanto mudaram) ou a taxa da sua transição
   (`How::Allocation`). Falta: a mancha desenhada no mapa; a razão da escolha da célula (a aptidão e a variável de
   iteração do CLUE), que o operador não guarda.
8. **Assistant:** destacar os nós durante a edição (além da seleção); as edições passando pela revisão do painel
   Changes; a avaliação de cada modelo com um conjunto fixo de tarefas.
   **Feito** (Unreleased): os nós que uma chamada de ferramenta nomeia (`node`, `nodes`, `id`, `rename`, o nó de
   `from` e de `key`, no seu `scope`) e o nó que uma mudança cria ganham um halo que se apaga em 2,5 s
   (`EditorState::pulse`); as edições já passavam pela revisão do Changes (`review::begin` em cada mudança e em
   `new_model`). A avaliação: `eval.rs`, 8 tarefas fixas (uma paisagem neutra, um limiar conferido célula a
   célula, um template mudado, um modelo consertado, uma explicação sem mudar o modelo, métricas, um rename que
   mantém as ligações, um laço), cada uma num modelo próprio, sem os problemas contados pelo app, conferida no
   que o assistente fez e na execução; "Evaluate this model" nas configurações do servidor, com o progresso e
   Stop, e a última nota de cada modelo guardada (`assistant-evaluations.json`). Falta: rodar as tarefas no
   modelo da HU Berlin (só na VPN; `eval::tests::hu_berlin_does_the_tasks`, à mão); mais tarefas (séries,
   alocações, a calculadora).
9. **Calculadora:** a prévia só da parte do mapa à vista (hoje o mapa inteiro em resolução reduzida); os tipos das
   portas de `out` (hoje "qualquer valor"); as camadas de uma pilha calculada no script no autocomplete
   (`s2 = s * 2`, então `s2.`); o autocomplete no campo do painel do nó.
   **Feito** (Unreleased): um retângulo arrastado num mapa da prévia é a parte calculada sozinha, com todas as
   células quando cabem na amostra (`algebra_tools::preview_in`, a janela lida do disco só nela), "Whole map" ou
   um duplo clique voltam ao mapa inteiro; um nome que o script dá a uma pilha mudada célula a célula (`s * 2`,
   `abs(s)`, `a if c else b`) tem as camadas dela no autocomplete (`stacks_before`; uma redução, uma camada ou um
   método não); o campo do painel do nó completa como a calculadora, as entradas conhecidas pelos valores da
   última execução. Falta: os tipos das portas de `out` (em `ops/algebra.rs` e no checker do grafo, onde outra
   sessão tinha mudanças abertas); a prévia seguir o zoom do viewer.
10. **My nodes:** bibliotecas do grupo e compartilhadas; as configurações de um nó do usuário como configurações
    (hoje entradas com valor próprio); os nós do usuário na busca rápida (`/`), no MCP e no Assistant.
    **Feito** (Unreleased): bibliotecas compartilhadas (as pastas de um grupo num drive compartilhado, um
    repositório clonado) nas configurações (`node_libraries`, "Add a shared library..."), os seus nós no painel de
    operadores numa seção de cada uma, o diálogo de salvar escolhendo em qual biblioteca, o painel do nó dizendo de
    qual ela é (`my_nodes::libraries`, `library_of`; um nome em duas: a do usuário primeiro). As entradas de valor
    (número, inteiro, sim ou não, texto) de um nó do usuário que nenhum nó alimenta são configurações: no painel do
    nó, sob Settings, com o próprio valor como dica e de volta a ele quando esvaziadas; no canvas, dobradas como as
    opcionais (`my_nodes::is_setting`). Os nós das bibliotecas na busca rápida (`my:nome`, antes dos operadores) e
    nas ferramentas do Assistant e do MCP: `list_operators` os lista, `describe_operator("my:nome")` diz as entradas,
    as configurações e as saídas, `add_node` com `op = "my:nome"` (ou `my:nome@2`) os acrescenta, as configurações
    dadas em `settings`. A biblioteca remota e as permissões: feitas depois (item 9 da lista de 2026-11-01).

## A interface para o cientista: funcionalidades modernas

Registro de 2026-10-31. O usuário comum é um cientista de biogeografia, geografia, biologia e ecologia: compara
mapas, roda cenários e simulações, publica figuras e precisa descrever os métodos. O que falta para isso, na
ordem de impacto (começando pelo 1 e pelo 3):

1. **Comparar mapas: cortina e vistas sincronizadas.** Uma cortina arrastada sobre o mapa ("antes" e "depois")
   e dois mapas lado a lado com o zoom e o arraste ligados. Quase toda análise compara: a cobertura de 2000 com
   a de 2020, um cenário com outro, o habitat atual com o futuro, o SDM com o observado. Hoje só as tabelas têm
   comparação lado a lado (painel Changes, Parameters).
   **Feito** (Unreleased): "Compare" no painel ao lado do mapa (Viewer e abas de mapa, `builder::Compare`): outra
   saída da execução ou outra camada da mesma (um passo de um laço, uma banda); "Curtain" (`map_view::Curtain`,
   `MapView::show_compared`: a outra à direita de uma linha arrastada pela alça, os nomes no alto, os valores das
   duas no canto) ou "Side by side" (duas `MapView` que se seguem por `builder::follow`, pelo lugar de uma grade na
   outra; uma cruz numa onde o ponteiro está na outra). As duas nas mesmas cores (`Coloring::shared`: as classes de
   ambas, ou a faixa das duas). Falta: o mapa da diferença (ou das transições) como terceira vista; a comparação no
   relatório (HTML e PDF); comparar mapas de sistemas de coordenadas diferentes sem `raster.match`.
   **Feito** (Unreleased): "Difference", a terceira vista (`difference.rs`, `builder::How`, `show_difference`): nos
   mapas de valores o outro menos este numa rampa divergente centrada no 0 (a média, a faixa, quantas células
   acima e abaixo); nos de classes as transições, uma classe cada com os nomes das legendas dos dois ("forest ->
   pasture"; a seta da fonte do app não tem "→"), as mais comuns primeiro (até 24, as outras juntas), as que não
   mudaram em cinza. Feita numa thread na grade do mapa (o outro lido onde estão as suas células; a cada poucas
   células acima de 16 milhões), desenhada na vista do comparado seguindo a do mapa (o clique e o Trace são da
   célula do mapa), e oferecida no "Figure...".
2. **Linha do tempo das simulações.** Os modelos com `control.repeat` (mudança de uso, autômatos celulares,
   CRAFTY, movimento animal) fazem um mapa por passo. O painel Simulation já passa os passos (play, setas) e o
   Viewer tem o slider das camadas de uma série; falta exportar em GIF ou MP4 (para aula e congressos), o play
   também no Viewer, e o passo escrito sobre o mapa.
   **Feito** (Unreleased): "Save as GIF..." no painel Simulation (os passos nas cores da simulação, ou os mapas
   de uma pilha de entrada, cada um nas suas cores) e no Viewer (as camadas de uma série, numa coloração só; de
   uma pilha ou de rasters, cada uma nas suas): `animation.rs`, o GIF feito numa thread, o mapa ampliado quando
   pequeno (até 480 px) ou lido a cada poucas células quando grande (até 800 px), o passo ("step 12 of 50") ou o
   nome da camada no canto, escrito na fonte do aplicativo (`ab_glyph`), a legenda (as classes ou a rampa e as
   pontas) embaixo, o último quadro mais longo, em loop. Falta: MP4 (precisa de um codificador; o `ffmpeg` se
   instalado?), o play no Viewer, a velocidade escolhida, e a animação como figura no relatório.
3. **Métodos e citações.** Um botão que escreve o parágrafo de métodos a partir do modelo: os operadores na ordem
   em que rodam, os parâmetros, a versão do Terraflux, e as referências de cada algoritmo (Soille e Vogt no MSPA,
   McRae no Circuitscape, Phillips no MaxEnt, Hesselbarth no landscapemetrics...), com a lista em BibTeX e RIS.
   Economiza horas, ajuda a reprodutibilidade e leva o Terraflux para os artigos (a citação dele também).
   **Feito** (Unreleased): `methods.rs`, a janela "Methods and citations" (menu File, painel Document, ctrl+K): o
   parágrafo (a versão, e uma frase por software reproduzido com os métodos usados e os seus artigos), o modelo
   passo a passo (as frases do documento, ou o começo da descrição do operador e as configurações dadas, com a
   nota), as referências em APA, copiadas ou salvas em BibTeX e RIS. As referências e os operadores de cada método
   são os da página "Cite Terraflux" do site, o software reproduzido o que as descrições dos operadores dizem; nada
   além disso é citado (os `refs` dos nós vão como estão escritos). Falta: os artigos das referências dos tutoriais
   (modelos neutros, MESS, blockCV, spThin, MigClim, ProtConn, Omniscape de McRae et al. 2016...); os métodos no
   relatório (HTML e PDF); `terraflux methods modelo.toml` na linha de comando e no MCP; o texto em português
   (feitos: ver o item 9 de "O que falta nas funções da 1.28.0").
4. **Desenhar no mapa.** Um retângulo ou polígono como a área de estudo (o recorte e a região dos downloads do
   `data.download`), pontos de amostragem, áreas protegidas propostas para os cenários: "desenhe a área e baixe o
   WorldClim só dela".
   **Feito** (Unreleased): "Draw on the map" no painel do Viewer (`draw.rs`, `MapView::drawing`): polígonos (um
   clique por vértice; Enter, duplo clique ou um clique no primeiro vértice fecha), retângulos (dois cantos),
   pontos e linhas, com Backspace (desfaz um vértice) e Escape; um clique desenha em vez de escolher a célula, e o
   duplo clique não reenquadra. "Add to the model" grava um GeoPackage ao lado do modelo (`drawn/<nome>.gpkg`, um
   `id` por feição, no sistema de coordenadas do mapa; um EPSG convertido pelo GDAL) e põe um `vector.read` dele no
   modelo, desenhado por cima do mapa. Falta: o recorte de um download do `data.download` pela área desenhada,
   editar uma feição já desenhada (mover vértices), atributos digitados, o preenchimento de polígonos côncavos
   (hoje só o contorno), e desenhar sobre o mapa de uma aba de mapa (hoje no Viewer).
5. **Figura pronta para publicação.** Um compositor: barra de escala, norte, legenda, grade de coordenadas, DPI,
   SVG e PDF, painéis A, B e C de vários mapas na mesma figura.
   **Feito** (Unreleased): "Figure..." no painel do Viewer (`figure.rs`): o mapa sozinho, com o comparado (nas
   cores que dividem) ou cada camada de uma pilha ou série (até 12; a série numa coloração só), em (a), (b)... com
   as legendas, em colunas; o título, a largura (85 mm, uma coluna; 180 mm, a página), 150 a 1200 DPI, a legenda (uma
   só à direita quando as cores são as mesmas, senão a de cada mapa embaixo dele; classes com os nomes da legenda,
   ou a rampa com mínimo, meio e máximo), a barra de escala (um quarto do mapa, em m ou km; em graus pela latitude
   do centro), a seta do norte (num mapa norte para cima), as coordenadas (linhas redondas, 1-2-5, rótulos em graus
   com hemisfério ou em metros com espaços). A figura é montada uma vez em pontos (formas) e escrita em SVG (o texto
   como texto, a editar no Inkscape; os mapas como PNG dentro, `href` e `xlink:href`) ou em PNG pelo tiny-skia (as
   letras como contornos da fonte, `ab_glyph`), com o `pHYs` da resolução. Conferido: o SVG lido pelo Qt (QGIS).
   Falta: PDF (o SVG abre no Inkscape e sai em PDF), um mapa de vetores por cima, o encarte de localização, a
   fonte escolhida, mover a legenda e a barra à mão.
6. **Catálogo visual de dados.** Os conjuntos que o `data.download` alcança (WorldClim, SoilGrids, MapBiomas,
   GBIF...) num catálogo com prévia, resolução, período e licença, em vez de só o operador.
   **Feito** (Unreleased): File > "Data catalog..." (e ctrl+K; `catalog.rs`): um cartão por fonte do
   `data.download` (WorldClim 2.1 e seus futuros CMIP6, CHELSA 2.1 e seus futuros, Copernicus DEM, SoilGrids 2.0,
   MapBiomas Brasil, Hydrography90m) com as cores do que traz, a resolução, os anos, a cobertura, a licença e a
   referência a citar (com DOI, "copy") e a página do dado; busca e filtro por tema (clima, relevo, solo, cobertura,
   rios). As escolhas de cada um (as variáveis bioclimáticas pelo nome, os meses, o modelo, cenário e período de um
   futuro, a propriedade, profundidades e estatística do solo, os anos do MapBiomas como `1985-2000, 2024`, a
   variável do Hydrography90m) e a grade: a de um mapa do modelo (`like`; de início o primeiro) ou uma caixa de
   longitudes e latitudes. "Add to the model" põe o `data.download` assim configurado, com a referência nas
   `refs` do nó (o Methods and citations a cita) e a licença no `why`. Licenças conferidas nas fontes (a do
   Hydrography90m é CC BY-NC 4.0 no registro FRED, CC BY 4.0 em outro: dito como está). Conferido: CHELSA bio12 e
   SoilGrids em duas profundidades baixados numa caixa de 1°. Falta: o GBIF (ocorrências, não rasters), uma prévia
   do dado na área (uma miniatura baixada), saber o que já está no cache, a caixa tirada de uma área desenhada ou
   de um vetor, e outros (ESA WorldCover, MODIS, Global Forest Change).
7. **Abrir no QGIS.** Exportar um projeto `.qgz` (ou um GeoPackage único) com todas as saídas, já com as cores e
   as legendas do viewer: muitos usuários terminam o mapa lá.
   **Feito** (Unreleased): "Export to QGIS..." (menu File, ctrl+K, e "Save output > In a QGIS project..." no
   Viewer; `qgis.rs`): as saídas de mapa e de vetor escolhidas (de início as do modelo e a mostrada), gravadas numa
   pasta (GeoTIFF, GeoPackage; um raster lido de arquivo referido onde está, sem cópia) com um projeto `.qgs` que as
   abre nas cores do Viewer: as classes como mapa "paletted" com os nomes da legenda (ou do produto, do MSPA), os
   valores numa rampa "singlebandpseudocolor" de 11 paradas (as de log espaçadas pelo log), os vetores por cima num
   símbolo simples (cores de Okabe e Ito), o sistema de coordenadas em WKT (um código EPSG convertido pelo GDAL,
   `terraflux_gdal::crs_wkt`), um `.qml` ao lado de cada mapa gravado. Conferido no QGIS 3.34 (PyQGIS: as camadas
   válidas, os renderizadores, as classes, o CRS do projeto). Falta: o `.qgz` (zip), as cores e espessuras dos
   vetores do Viewer ("Vectors over the map"), os mapas por cima com sua opacidade, a legenda contínua com rótulos
   escolhidos, e um GeoPackage único com os rasters dentro.
8. **O modelo como script Python do landschaft.** Um script equivalente ao modelo, para o artigo, o orientador ou
   um revisor que pede o código.
   **Feito** (Unreleased): File > "Python script (landschaft)..." (e ctrl+K; `python.rs`): o modelo como um script
   do landschaft, uma chamada por nó na ordem em que o modelo os roda, a nota e o porquê como comentários, as
   configurações como argumentos pelos nomes do landschaft (rows/columns → nrow/ncol, range → autocorr_range,
   window → window_size...), as saídas por `ls.save`. Traduzidos à mão e conferidos: leitura e escrita (rasters,
   vetores, tabelas), o raster.calc (o mesmo script, rodado em numpy por um ajudante escrito no arquivo,
   `python_calc.py`: comparações encadeadas, and/or/not, a if c else b, None, nb*, estatísticas de pilha, x.mean()),
   o raster.reclassify (a tabela do `ls.reclassify`, faixas com as duas pontas dentro), MSPA, SPA, FAD, FAD-MS, FOS,
   Accounting (o foreground por `ls.reclassify(foreground=...)`) e os modelos neutros (com a semente 1 e os padrões
   do Terraflux, p.ex. `rescale=False` no random cluster); 33 operadores cujos argumentos têm os mesmos nomes vão
   por nome. Conferido rodando os dois: os mesmos mapas (as classes idênticas, os valores até a precisão do float32
   que o Terraflux grava). Falta: os contêineres (loops, each, switch), os modelos de mudança (Dinamica, CLUE-S,
   PLUS), SDM, priorização, métricas de paisagem, conectividade e o resto dos ~430 operadores, que ficam como
   comentários com suas configurações.
9. **Paletas para daltônicos.** viridis e cividis à mão, e uma prévia do mapa como um daltônico o vê (deuteranopia,
   protanopia, tritanopia); as revistas cobram cada vez mais.
   **Feito** (Unreleased): as rampas cividis e PuOr (laranja a roxo, centrada no 0) nas cores do Viewer; View >
   Colour vision (`palette::Vision`, as matrizes de Machado et al. 2009 em RGB linear) em todo mapa, legenda e
   imagem, dito na barra do alto até voltar ao normal. As classes já começavam pelas cores de Okabe e Ito. Falta: os
   gráficos (egui_plot) na visão escolhida; avisar quando duas classes de um mapa ficam parecidas demais para um
   daltônico.
10. **Uma célula, todas as camadas.** Clicar numa célula e ver o valor em cada mapa do modelo e, numa pilha
    temporal, o gráfico ao longo do tempo (o Trace já explica uma célula passo a passo; isto é a consulta rápida).
    **Feito** (Unreleased): "This cell in every map" no painel ao lado do mapa (`builder::cell_everywhere`): o valor
    da célula clicada em cada mapa da execução, no mesmo lugar pelas coordenadas, guardado enquanto a célula e os
    mapas são os mesmos; um nome mostra o mapa, "Copy" copia a lista. O gráfico ao longo das camadas de uma pilha já
    existia. Falta: as camadas de todas as pilhas (hoje a primeira, ou a banda escolhida).

## O que falta nas funções da 1.28.0

Registro de 2026-11-01. As sobras ("Falta:") dos itens de "A interface para o cientista", escolhidas pelo usuário
como o próximo bloco, nesta ordem (um commit por item):

1. **Comparar mapas:** o mapa da diferença ou das transições como terceira vista, e a comparação no relatório
   (HTML e PDF). **Feito** (Unreleased) a terceira vista (ver o item 1 acima); a comparação no relatório vai
   com o item 2.
2. **Relatório:** levar para o HTML e o PDF os métodos e citações, os selos de verificação, a comparação e as
   animações. **Feito** (Unreleased) os métodos e os selos: `report::Extras` (os `Methods` e os `Check` da
   execução; no `terraflux report` por `checks::of_run`, sem `Session`), a saúde no alto com uma linha por check,
   o selo no passo de cada check e nos contêineres (o pior), e a seção "Methods and citations" no fim (o
   parágrafo, as referências em APA, as dos nós). No PDF, o selo é um retângulo na cor com o rótulo em branco.
   **Feito** (Unreleased) as animações: uma série de mapas (os passos de um laço) é um GIF animado no HTML
   (`animation::gif`, em memória; até 40 passos, o primeiro e o último entre eles, numa coloração só) e quatro
   passos lado a lado no PDF (`Pdf::strip`). **Feito** (Unreleased) a comparação: guardada no modelo, não no
   estado do Viewer (o relatório só vê o modelo): o `view.plot` com a entrada `compared` e a configuração
   `compare` (`curtain`, `side by side`, `difference`); a aba do plot abre assim (`Compare::plotted`, de novo
   quando o modelo muda), e a figura do relatório mostra os dois mapas nas cores que dividem e, com
   `difference`, a diferença com o que diz e a legenda (`report::compared`, `difference::make`).
3. **Animação:** MP4, o play no Viewer e a velocidade escolhida.
   **Feito** (Unreleased): o MP4 pelo ffmpeg quando instalado (`animation::ffmpeg`, no PATH ou em
   `TERRAFLUX_FFMPEG`; os quadros em RGBA pela entrada do ffmpeg, H.264 com `yuv420p`, os lados tornados pares,
   `-threads 4`, sem janela de console); "Play" no Viewer (`Shown::playing`, do último ao primeiro de novo); a
   velocidade em quadros por segundo (`Animations::fps`, `speed`), a mesma para tocar (Viewer e Simulation) e
   para o GIF e o vídeo. Falta: um codificador sem o ffmpeg (o `openh264`?).
4. **Desenho → dados:** o `data.download` recortado pela área desenhada; editar vértices; digitar atributos.
   **Feito** (Unreleased): a entrada `area` do `data.download` (polígonos; fora deles sem dado, pelo `-cutline`
   do gdalwarp com a área gravada em `area.gpkg` na pasta do cache; a área entra no hash do cache); sem `like` e
   `extent`, a grade cobre a área em graus, alargada a células inteiras (`cellsize`, padrão o lado maior em 500
   células). No catálogo, "Cut to the area" e a grade "the area's bounds"; no desenho, "Add, and data for it..."
   adiciona e abre o catálogo assim. Vértices: arrastar (`draw::drag`, antes do arrasto do mapa), o ponto no meio
   da aresta cria um, o botão direito tira (`Sketch::move_vertex`, `insert_vertex`, `remove_vertex`, `hit`).
   Atributos: `Sketch::columns` e `values`, uma tabela na seção do desenho; números quando todos os valores são.
   Falta: editar os vértices de uma camada já lida (só os do desenho em curso).
5. **Catálogo:** GBIF, prévia do dado na área, saber o que já está no cache, e ESA WorldCover, MODIS e Global
   Forest Change.
   **Feito** (Unreleased): fontes `esa_worldcover` (ladrilhos de 3° pelo canto sudoeste, 2020 v100 e 2021 v200,
   COG), `global_forest_change` (Hansen v1.13, ladrilhos de 10° pelo canto noroeste; não são COG: uma janela lê
   linhas inteiras do ladrilho, dito no cartão) e `modis` (MOD13Q1 NDVI/EVI pelo STAC do Planetary Computer: o
   composto de 16 dias começado na data ou o último antes dela; o token anônimo é o da conta e contêiner do
   arquivo, `/sas/v1/token/{conta}/{contêiner}`; o da coleção dava 403). O `data::plan` acha sem rede a pasta do
   cache, a grade e as camadas (os ladrilhos que existem e o MODIS são resolvidos só no warp, `sources_of`), e o
   catálogo diz o que já está no cache. "Preview": a primeira camada numa cópia grossa da grade (200 células no
   lado maior) numa thread, mostrada num MapView; para o GBIF, a contagem da API (`limit=0`). O GBIF é um cartão
   que põe `sdm.download_gbif` (espécie, países, anos, a área). O MODIS de cobertura da terra (MCD12Q1) ficou de
   fora: só no LP DAAC, com conta Earthdata.
6. **Figura:** PDF, vetores por cima, encarte de localização.
   **Feito** (Unreleased): `Drawing::pdf` (uma página do tamanho da figura; Helvetica WinAnsi pelo `report::ansi`
   e as larguras do `report::helvetica_width`, o texto de pé por `Tm`; as opacidades por ExtGState; os mapas como
   imagens RGB com SMask para as células sem valor). Novas formas `Path` (anéis pela regra par-ímpar, ou linhas) e
   `Clip` (recorte num retângulo: `clipPath` no SVG, `Mask` no tiny-skia, `re W n` no PDF). `Panel::vectors`: as
   camadas de "Vectors over the map" (e as de um `view.plot`), reprojetadas para a grade do mapa, nas cores do
   estilo (`vector_tiles::colours`). O encarte (`Layout::inset`, ligado por padrão): a terra do Natural Earth
   1:110m embutida (`assets/ne_110m_land.bin`, 20 KB, centésimos de grau), longitudes encolhidas pelo cosseno da
   latitude do meio, seis vezes o vão do mapa (24° pelo menos), o mapa em vermelho (um anel quando pequeno
   demais). Falta: a fonte escolhida, mover a legenda e a barra à mão, o encarte num mapa sem CRS conhecido.
7. **QGIS:** `.qgz` e as cores dos vetores do Viewer.
   **Feito** (Unreleased): o projeto em `.qgz` (padrão; um zip escrito à mão, deflate do `miniz_oxide`, CRC32 da
   figura) ou `.qgs`, escolha na janela. Os vetores com o estilo do Viewer (`preview_overlays`): cor, preenchimento
   e espessura (px × 0,26 mm), ou por um atributo: `categorizedSymbol` (textos, inteiros até 24 classes, as cores de
   `vector_tiles::colours`) ou `graduatedSymbol` (7 faixas na rampa sequencial). Os vetores sobre o mapa vêm
   marcados. Conferido no QGIS 3.34 (PyQGIS: o `.qgz` lido, o mapa paletted, os vetores categorizados nas cores do
   Viewer). Falta: rótulos dos vetores, regras (`ruleRenderer`) e símbolos de pontos além do círculo.
8. **Script Python:** os contêineres (laços, each, switch), os modelos de mudança, SDM, priorização e métricas
   (hoje ficam como comentários).
   **Feito** (Unreleased): `python.rs` com escopos (`Scope`: os nós feitos e os `@nomes` de cada corpo, depois os de
   fora) e um `Writer` (nomes Python únicos por `fresh`): o laço vira `for step in range(...)` (carry no fim do passo,
   collect em listas, until um `break`), o each um `for index, (name, item) in enumerate(_elements(...))` com
   `_gather`, o switch `if/elif/else` (`_case`, `case_3` aceita "3"), o grupo o corpo no lugar; um nó do corpo sem
   equivalente vira comentário e a referência a ele `None` (o contêiner fica). Ajudantes em `python_flow.py`
   (`_stack`, `_table`, `_elements`, `_gather`, `_case`). Traduzidos: stack.of, table.of, sdm.prepare/background/fit
   (MaxNet, GLM, floresta)/maxnet_predict/importance/response_curves, metrics.class/landscape/patch
   (`ls.landscape_metrics`, o tipo com " metric"), prioritize.optimize (a cadeia do `problem()`; o que não cabe é
   recusado, não aproximado) e zonation, markov.fit, woe.fit/probability, change.transitions, validate.three_map,
   surface.terrain/tpi/vrm/curvature. Conferido rodando os dois (t13, t15, t17, t35, t46 e um each sobre uma pilha):
   os mesmos mapas; achado e corrigido no caminho o `nbsum` do `python_calc.py` (média × 9 não é exata). Nos
   templates: 390 traduzidos, 170 restantes. Falta: a alocação do Dinamica, CLUE-S e PLUS passo a passo (o
   landschaft só a roda no modelo inteiro, `ls.simulate.run`, o que o script diz), vetores (pontos, polígonos,
   extract), tabelas (calc, aggregate, filter), o GLM do CLUE-S, as zonas e as penalidades de conectividade no
   prioritize.
9. **Métodos:** `terraflux methods` na linha de comando e no MCP; o texto em português.
   **Feito** (Unreleased): `methods::Lang` (en, pt) e `of_in`: o parágrafo, os nomes dos 24 métodos (`name_pt`), os
   títulos e a referência do Terraflux em português; cada etapa pela nota do nó, senão as configurações (as
   descrições dos operadores e as frases do documento são em inglês). A escolha na janela (English / Português);
   `terraflux methods modelo.toml [-l pt] [-f text|bibtex|ris] [-o arquivo]`; a ferramenta `methods` do MCP e do
   assistente do aplicativo (`tools::methods`). Falta: as frases das etapas em português (as do documento,
   `phrases.rs`), o relatório em português.

## Sobras das partes recentes

Registro de 2026-11-01. As "Falta:" das partes já lançadas (as de "A interface: o que falta nas partes já feitas" e
de "O que falta nas funções da 1.28.0"), escolhidas pelo usuário como o próximo bloco, nesta ordem (um commit por
item):

1. **Métodos e relatório em português:** as frases das etapas em português (`phrases.rs`) e o relatório inteiro.
   **Feito** (Unreleased): `phrases::PHRASES_PT`, as frases dos 36 operadores em português (os mesmos
   `{configurações}`, `<entradas>` e `[partes]` das inglesas, conferido num teste), nos métodos e no documento;
   `pt.rs`, os textos do relatório pela sua forma inglesa (`Lang::t`, `Lang::f` com os valores), o que vale também
   para as verificações (`Verdict::words_in`, `checks::health_in`) e a comparação de dois mapas
   (`difference::make_in`); `Document::of_in` (as etapas, os tipos das entradas, sim e não, os intervalos dos
   parâmetros) e `report::Extras::of_in` (a língua do relatório, os métodos nela). No aplicativo, Export report
   pergunta a língua (In English / Em português); `terraflux report modelo.toml -l pt`. As descrições dos
   operadores e das suas configurações continuam em inglês.
2. **Vista Document:** frases para os operadores que ainda não têm (hoje são 36) e tabelas com mais de uma coluna.
   **Feito** (Unreleased): mais 73 frases (109 ao todo), em inglês e português, as dos operadores mais usados nos
   templates e nos tutoriais (um teste lê as frases de todos os templates nas duas línguas); um texto vazio conta
   como não dado. As tabelas de várias colunas editadas célula a célula (`node_view::grid`, `Grid`, `grid_ui`): listas
   de linhas, listas de registros e registros por nome, no documento e no painel Node; as colunas na ordem que o
   modelo guarda (alfabética: o serde_json sem `preserve_order`). Falta: frases para os outros ~320 operadores;
   tabelas dentro de tabelas (seguem como texto).
3. **Painel Changes:** casar as linhas das tabelas por uma chave, não pela posição.
   **Feito** (Unreleased): `review::key_column` escolhe a chave (uma coluna das duas tabelas de números inteiros ou
   nomes, sem repetidos, com algum valor em comum; `id`, `key`, `class`, `value`... primeiro), `review::matched_rows`
   casa as linhas por ela (sem chave, pela posição); a chave vem primeiro, "rows matched by …" acima, uma linha só
   de antes em vermelho e só de agora em verde, as células comparadas com as da linha casada.
4. **Grupos e laços:** ver no viewer o mapa de um nó de dentro dos corpos de `control.repeat`/`switch`, e congelar
   um nó de dentro de um grupo.
   **Feito** (Unreleased): o `control.repeat` dá o que os nós do corpo deram no último passo (`node.port`, como o
   grupo), o `control.switch` o que o ramo que rodou deu por ele (`ramo.node.port`); `trace::shown_key` e
   `builder::shown_at` levam o viewer até eles (nomeado `years/grow (the last step)` num laço), e o documento mostra
   os passos do ramo que rodou. Congelar um nó de dentro de um grupo (grupos em grupos também): pelo lugar,
   `group/node` (`terraflux_exec::frozen_places`, a pasta `<modelo>.frozen/group.node/`, `Cache::hold` no `inner`),
   no app e no `terraflux run`; segurar ou soltar esquece o que o grupo de cima deu. Falta: o que só o nó congelado
   lê dentro do grupo ainda roda; o `control.each` (o último elemento) e um passo escolhido do laço no viewer.
5. **Cenários:** a matriz de transições inteira e mais de um resultado comparado por vez.
   **Feito** (Unreleased): `params_view::transitions` (as classes de um e de outro, as células de cada par) e "the
   whole transition matrix" sob cada cenário (as linhas a classe do primeiro, as colunas a deste, a diagonal fraca,
   os totais); as saídas comparadas marcadas numa lista (`State::outputs`), cada cenário rodado uma vez até todas (o
   cache dele entre elas), um bloco por saída (`Comparison`), também pelo Many runs (`compare`: as saídas).
6. **Trace:** a mancha desenhada no mapa e a razão da escolha da célula.
   **Feito** (Unreleased): `How::Patch` guarda as células da mancha (até `PATCH_DRAWN`), `trace::patch_outline` os
   lados da borda, e o nome do cartão desenha a mancha no mapa do viewer (`MapView::patch`; o retângulo das linhas e
   colunas quando é grande demais; outra célula escolhida a solta). A razão da escolha numa alocação, uma linha
   "why" por mudança: no `lulcc.step` a aptidão da célula para cada classe, a posição da classe que ela tomou entre
   as células e a elasticidade da classe que ela tinha; no `dinamica.step` a probabilidade da transição na célula e
   a sua posição. Falta: a variável de iteração do CLUE, que o C++ do landschaft não devolve (`CluesOut`).
7. **Depuração:** um breakpoint de verdade num nó escolhido (hoje o laço só para entre passos) e o Stop dentro das
   chamadas do C++.
   **Feito** (Unreleased): o breakpoint: `runner::Gate` guarda os lugares (`node`, `laço/node`) e a execução para
   antes do nó (`Observer::node`), com o que rodou antes mostrado; "Breakpoint" no painel Node, um ponto vermelho
   no canvas, "paused before …" com **Next node** (para antes do próximo nó) e **Continue** (até o próximo
   breakpoint). Da sessão, não do arquivo do modelo. Falta: o Stop dentro das chamadas do C++ (o MSPA de uma
   telha, o `landscape.mspa` inteiro, o `circuit.pairwise`, o Omniscape) precisa de um gancho de cancelamento no
   C++ do landschaft (como o `SolveWatch` do `opt_solve_watched`), registrado no IDEAS.md de lá; hoje o Stop
   solta a execução e o operador termina em segundo plano, o resultado descartado.
8. **Calculadora:** os tipos das portas de `out` no checker do grafo.
   **Feito** (Unreleased): `Catalog::script_outputs` (o registro do ops responde pelo `raster.calc` com
   `algebra::out_types`, uma inferência sobre a árvore do script: número, mapa, pilha, rasters lidos, desconhecido)
   e o checker a usa em `port_type` com os tipos do que chega às entradas do nó (guardados por nó); uma saída ligada
   onde não cabe é recusada no check. `terraflux_graph::script_types` dá os tipos às respostas das ferramentas
   (`its outputs: calc.share (number)`). Os 52 modelos e templates do site continuam sem erros.
9. **My nodes:** biblioteca remota (URL ou repositório git) e permissões.
   **Feito** (Unreleased): `node_remotes.rs`. Uma biblioteca remota é um repositório git (`git clone --depth 1`,
   depois `git pull --ff-only`, sem janela de console e sem pedir senha) ou uma pasta na web lida pelo seu
   `index.txt` (as versões que faltam, cada uma conferida como nó antes de gravada; o índice escrito pelo app em
   cada biblioteca onde salva ou tira um nó). A cópia fica em `remote nodes/<nome>-<hash>` nos dados do app,
   puxada quando o app abre e com **Update** (`settings.node_remotes`), e só é lida: `Shelf::may_save` a recusa
   dizendo onde acrescentar. Permissões: o `library.toml` de uma biblioteca (`name`, `writers`); quem não está em
   `writers` (o login, ou `TERRAFLUX_USER`) não salva nem tira nós, o diálogo a mostra desabilitada com a razão e
   um nó dela editado vai para a biblioteca do usuário. Testes: o endereço (git ou web), os writers, uma pasta
   servida em 127.0.0.1 e um repositório local clonado e puxado; no teste de UI, a seção da biblioteca remota.
10. **A priorização multi-ação depois da atualização do landschaft** (pedido de 2026-11-01): conferir que o
    `prioritize.actions` e o `prioritize.actions_raster` continuam iguais ao landschaft novo (o submódulo, o shim, as
    referências). **Feito** (Unreleased): a mudança do landschaft 1.58.1 (81f5281) é só do Python (mais rápido, e
    alvos relativos de rasters que não falham mais quando uma feição não tem quantidades); as referências de
    rasters regeneradas saem iguais. Um caso novo (`no_amounts`: a camada de uma feição toda 0) dá as mesmas
    tabelas, alvos e ótimo do landschaft; faltava só o aviso de feição descartada ("The following features are not
    represented…"), que o Terraflux perdia. Um teste de ids das unidades fora de ordem, com lacunas e inteiros (o
    `test_table_ids_in_any_order` do landschaft); o submódulo em 1.58.1 (nenhuma mudança no C++).

## As pontas de 2026-11-02

Pedido de 2026-11-02: as abas que aparecem quando fazem sentido, e as pontas que sobravam (um commit por item).

1. **As abas Node, Prepare e Calculator só quando fazem sentido.** **Feito** (Unreleased): o programa abre com
   Model e Document (`default_layout`); `open_due_with` abre o Node com o primeiro nó ou template, o Prepare (atrás
   dele, o Node na frente) e o Viewer com os dados, a Calculator só pedida (`ask_calculator`). Um layout guardado
   com elas não as traz (`comes_when_due`, tiradas em `restore_layout`); fechada, a aba fica fechada até outro
   modelo abrir (`panes_due_again`). O foco fica onde estava (`open_behind`).
2. **Frases para os ~320 operadores sem uma.** **Feito** (Unreleased): todas, em inglês e português (431 cada).
3. **Selos no relatório e `check.*` como operadores próprios.** Os selos já estavam (1.30.0, HTML e PDF).
   **Feito** (Unreleased): `ops::checks`, `check.range`, `check.no_nodata`, `check.area_kept`, `check.accuracy`,
   `check.demand` (cada um dá `ok`, para a execução salvo com `warn`, a mensagem dada ou uma própria);
   `checks::is_check` em todo lugar que olhava só o `check.assert` (o relatório, o documento, o editor, o
   `terraflux test`); `checks::what` e `mark` descrevem um sem expressão.
4. **Calculadora: a prévia que segue o zoom e o autocomplete das pilhas.** O autocomplete já estava.
   **Feito** (Unreleased): `CalcState::follow` ("Follow the viewer"), a janela da prévia pela parte do mapa à vista
   no Viewer (`MapView::visible`, `calc_view::window_in`), de novo quando ela muda; um retângulo desenhado à mão
   desliga.
5. **Bibliotecas de nós do grupo.** **Feito** (Unreleased): `my_nodes::reachable` (os nós de todas as
   bibliotecas, o nome qualificado `lab/ndvi` quando está em duas), `my:lab/ndvi` no `add_node`, no painel de
   operadores, na busca rápida, nos comandos e no `list_operators`; a ferramenta `save_my_node` do assistente (o
   nome, a descrição, a biblioteca). `my_nodes::get` diz as bibliotecas quando não acha uma.
6. **MP4 sem ffmpeg.** **Feito** (Unreleased): `openh264` 0.9 (compilado da fonte, 4 threads, conteúdo de tela)
   e um MP4 escrito à mão (`mp4.rs`: `ftyp`, `moov` antes do `mdat`, `avcC`, `stts`, `stss`, `stsz`, `stco`), os
   lados pares com branco, o último quadro quatro vezes mais longo; um teste decodifica de volta e confere a cor.
   Nota: o OpenH264 compilado da fonte não vem com a licença de patentes do H.264 que a Cisco dá ao seu binário.
7. **Catálogo: GBIF, prévia, cache, WorldCover, MODIS e GFC.** Quase tudo já estava. **Feito** (Unreleased):
   MODIS além de NDVI e EVI (`data::MODIS`): a temperatura da superfície de dia e de noite (MOD11A2), a área
   queimada (MCD64A1) e a produção primária bruta e líquida (MOD17A3HGF), cada uma pelo seu produto no Planetary
   Computer e citada pela sua referência (`catalog::reference_of`). O MCD12Q1 (cobertura) não está no Planetary
   Computer.
8. **QGIS: rótulos e regras.** **Feito** (Unreleased): `qgis::VectorOptions` por camada vetorial na janela (o
   atributo dos rótulos, o símbolo dos pontos entre oito, "as rules"): `<labeling type="simple">` com
   `labelsEnabled`, o símbolo em `SimpleMarker`, e o `RuleRenderer` (uma regra por classe ou faixa, os textos com
   aspas dobradas, e uma `ELSE`). Conferido no QGIS 3.34 (PyQGIS): item 11 de "O próximo bloco de 2026-11-03".
9. **Script Python: alocação passo a passo, vetores e tabelas.** **Feito** (Unreleased), em parte a alocação:
   as tabelas em pandas (`table.filter`, `calc`, `aggregate`, `select`, `sort`, `join`, `pivot`; as expressões pelo
   `_calc`, os textos pela ordem), os vetores em geopandas e no landschaft (`vector.read` com a tabela,
   `rasterize`, `zonal`, `extract`, `filter`, `calc`, `buffer`, `clip`, `dissolve`, `reproject`, `join`,
   `spatial_join`, `overlay`, `points` com os mesmos sorteios PCG64, `from_table`, `polygonize`, `distance`,
   `grid_zones`, `geometry`), ajudantes em `python_flow.py`; o GLM (`glm.fit/predict/coefficients` pelo
   `LogisticRegression`), as peças do Dinamica (`cost_map`, `destinations`, `pathways`, `roads`,
   `continuous_patcher`, a semente como o `step_seed`), as demandas (`demand.table`, `demand.from_maps`) e as
   penalidades de conectividade do `prioritize.optimize`. Conferido: um modelo de 55 nós de tabelas e vetores dá os
   mesmos 37 CSV que o Terraflux (1e-9), e as estradas do Dinamica iguais (o `ls` de entrada e saída simulado: o
   landschaft não está instalado com o núcleo compilado aqui). Recusados: `vector.density` (o landschaft não tem),
   `vector.exact_sum` (só privado), `geometry` geodésica. Falta: `dinamica.step` e `cars.step` (o `ExpanderPatcher`
   e o `CARS` só rodam o modelo inteiro), `lulcc.step` (o `CLUE.allocate` existe, mas a elasticidade, as transições,
   o histórico e as restrições ficam no código privado), `plus.demand` e `demand.regional`, e as zonas do
   `prioritize.optimize` (o landschaft as tem, mas as pilhas, os alvos e os orçamentos por zona pedem uma
   conversão).
10. **A lista das falhas dos lotes num painel.** Já estava: o painel Node do primeiro nó da cadeia lista os
    arquivos que falharam com o motivo, "Run again" e "Copy the list" (`Observer::failures`); o Many runs dá o erro
    de cada rodada. As "Falta" de "Lotes e laços" eram de antes disso.
11. **A avaliação do assistente com o Qwen da HU.** Rodada em 2026-11-02 (`eval::tests::hu_berlin_does_the_tasks`,
    o modelo `llm3`): 5 de 10 na primeira vez, 2 de 10 nas seguintes; a explicação e a revisão do método sempre
    passam. As falhas não são do raciocínio: o modelo escreve a chamada como texto (`edit_node(node="a", ...)`,
    um bloco ```json com `name` e `arguments`, `<invoke_name ...>`, `<list_operators><query>...`), o servidor
    responde `finish_reason: stop` sem `tool_calls`, e a tarefa acaba sem ferramenta. A mesma pergunta sozinha às
    vezes passa (a das métricas: 13 ferramentas, o modelo montado e rodado), e um pedido simples com uma ferramenta
    sai certo 20 de 20 vezes, com e sem streaming; o "Few tools" não ajuda (2 de 10). Ideia: reconhecer essas
    chamadas escritas como texto e rodá-las (os formatos acima), ou pedir de novo quando a resposta termina
    anunciando uma ferramenta sem chamá-la. **Feito** (Unreleased): item 1 de "O próximo bloco de 2026-11-03".

## O próximo bloco de 2026-11-03

Pedido de 2026-11-03: as partes abertas do assistente e do MCP e as pontas das partes já lançadas, nesta ordem (um
commit por item):

1. **Chamadas escritas como texto.** **Feito** (Unreleased): `written.rs`. Quando a resposta não traz `tool_calls`,
   `written::calls` acha no texto as chamadas das ferramentas oferecidas: em Python (`edit_node(node="a", ...)`, no
   começo de uma linha, os literais do Python lidos, os posicionais pelos `required`), em JSON (um bloco ```json com
   `name` e `arguments`/`parameters`, os argumentos como texto, `{"function": ...}`, `tool_calls`, listas; o
   `<tool_call>` do Hermes) e em tags (`<function=nome><parameter=chave>`, `<invoke name="...">`,
   `<invoke_name>`, `<nome_da_ferramenta><chave>...`), os valores pelo tipo do esquema. As chamadas viram
   `tool_calls` de verdade na conversa (o modelo as vê como chamadas) e saem do texto mostrado. Uma resposta que
   anuncia uma ferramenta sem chamá-la (o nome no último parágrafo com palavras de intenção, ou o texto terminando
   em dois-pontos) é pedida de novo uma vez (`written::ask_again`). Testes: os formatos, um servidor falso em
   127.0.0.1 que responde com uma chamada escrita e outro que só anuncia.
2. **Avaliação antes e depois** com o Qwen da HU (VPN) e um modelo grande, guardada aqui.
   **Feito** (Unreleased), só com o Qwen da HU (llm3): não há chave de um modelo grande no chaveiro, e a HU só serve
   o llm3. As 10 tarefas do `eval.rs`, uma rodada por vez. Antes (2026-11-02): 5/10, depois 2/10 (as chamadas
   escritas como texto, a tarefa terminando sem ferramenta). Depois do item 1: 4/10 (duas tarefas paradas pelo limite
   de 10 minutos: o servidor recusando pedidos seguidos), 7/10 e 6/10. O que as rodadas mostraram e entrou aqui: os
   pedidos à HU espaçados (`llm::pace`, 15 por minuto em hu-berlin.de); um `</think>` sem `<think>` tirado do texto
   (também partido entre dois pedaços); o formato `<function_calls><function_call><parameter name="ferramenta">` do
   Qwen e as tags dentro de `tool_call(s)`/`function_call(s)`/`tool_use` lidas pelo nome da ferramenta e pelas
   propriedades do esquema (`written::loose`); uma cerca de código vazia deixada pela chamada tirada; uma ferramenta
   citada no que parece código, e a resposta que termina prometendo agir ("Let me build this model now.", "Vou ..."),
   pedidas de novo. `Outcome::said` guarda a conversa, impressa nas tarefas que falham. Falhas que ficam, do próprio
   modelo: o limiar trocado pela média (0.6 pedido), a saída do nó consertado não rodada, métricas sem o
   `nlm.random_cluster` pedido.
3. **Mais regras de método** (`advice.rs`): o `steps` do `markov.fit` contra as datas, `woe.fit` sem distância,
   resistência com zeros ou negativos, priorização sem `prioritize.evaluate`, SDM sem thinning dos registros.
   **Feito** (Unreleased): os anos lidos nos nomes dos arquivos dos `raster.read`, dos nós e das entradas
   (`year_in`, quatro dígitos de 1900 a 2100); `steps` que não divide os anos (conselho, com o `steps` anual e o
   divisor mais próximo) ou t0 depois de t1 (alerta). As distâncias procuradas acima dos `drivers` (operadores de
   distância e custo, nomes com "dist" ou "cost" em nós, portas, arquivos e camadas carregadas); um arquivo ou uma
   entrada sem nome que diga nada não é acusado. A resistência das entradas carregadas (portas com "resistance" e o
   `habitat` do Circuitscape quando é resistência) com células de 0 ou menos: alerta. `sdm.fit` sem
   `sdm.one_per_cell`, `sdm.thin`, `sdm.prepare` nem `sdm.clean_occurrences`; `prioritize.optimize`, `marxan`,
   `actions` ou `actions_raster` sem `prioritize.evaluate`. Os templates continuam sem alertas (Dinamica EGO,
   CLUE-S, PLUS, CRAFTY e três de priorização recebem conselhos).
4. **Os conselhos no app:** no painel Problems (nível "advice") e no relatório.
   **Feito** (Unreleased): `Session::advice`, calculado de novo quando a revisão do modelo ou a leitura dos dados
   muda; "The method" no painel Problems (alerta em amarelo e conta entre os warnings, conselho em ○ com a cor de
   destaque, os nós como links, *Ask the assistant*); `report::Extras::with_advice` no HTML e no PDF ("On the
   method" / "Sobre o método", selos warning/advice), no Export report do app e no `terraflux report` (com os dados
   lidos quando `--run`).
5. **Dicas por operador** num campo `advice` do próprio operador, para `conn.*`, hidrologia, InVEST, agentes e REDD+.
   **Feito** (Unreleased): `OpSignature::advice` (serde, omitido quando vazio) e `.advice(...)`; como vários
   operadores são registrados por tabelas (essentials, hydro), as dicas ficam em `terraflux_ops::tips` e entram na
   assinatura no `Registry::add` (a do próprio operador vale primeiro). As 19 de `op_notes.rs` foram para lá, mais 25:
   grafos de manchas, ProtConn, mudança do ECA, importância dos nós; preenchimento, acumulação, rios, HAND, TWI,
   bacias; carbono, qualidade do habitat, produção de água, SDR, NDR, polinização; `agents.update`, CRAFTY; a
   vulnerabilidade, o ajuste, a avaliação e a alocação do REDD+ e as emissões.
6. **Protocolo e línguas:** saída estruturada no `check_model` e no `run_model`, elicitation, guias em português.
   **Feito** (Unreleased): `tools::output_schema` e `tools::structured` (os dados tirados do texto da resposta: os
   erros, warnings e conselhos do check; a duração, os arquivos, os warnings, os conselhos ou o erro da rodada),
   `outputSchema` no `tools/list` e `structuredContent` no `tools/call` do MCP. Elicitation: o `initialize` guarda se
   o cliente declara `elicitation`; o servidor manda `elicitation/create` e espera a resposta pela entrada padrão
   (`Client`: os pedidos em aberto por id, a resposta entregue à thread que espera; o `set_input` roda numa thread
   como o `run_model` quando o cliente pergunta). O `set_input` de um mapa de classes (2 a 24 inteiros,
   `tools::classes_of`) sem nota pergunta o nome de cada classe e os guarda na nota ("Classes: 1 = forest; ...").
   Os seis guias de método em português (`guide::GUIDES_PT`, `guide_in`, `language: pt` na ferramenta), os mesmos
   operadores e templates citados (um teste); os de construção do modelo ficam em inglês.
7. **Script Python:** `dinamica.step`, `cars.step`, `lulcc.step`, `plus.demand`, `demand.regional`, as zonas do
   `prioritize.optimize`.
   **Feito em parte** (Unreleased): as zonas do `prioritize.optimize` (`zone_costs` como o custo de cada zona, uma
   camada por zona com o nome dela; `features` com `contribution`; `mandatory_allocation` como
   `add_mandatory_allocation_constraints()`; `solution` = `.zones`, `allocation` = `.selection` com as zonas como
   camadas, `coverage`), conferidas no landschaft 1.56 instalado. Um portfólio de zonas e as outras entradas de zona
   (`zone_features`, `contribution_maps`, `zone_locked_*`) ficam de fora, ditas no script. **Esperando o landschaft:**
   os passos (`dinamica.step`, `cars.step`, `lulcc.step`) só existem dentro do `simulate` do modelo inteiro, e o corpo
   de um passo chama `_core.sim_patches` / `sim_cars`, privados; pedido no IDEAS.md do landschaft ("Um passo de cada
   alocação, público"). `plus.demand` e `demand.regional` (`ls.simulate.Regional` é público) só servem a esses
   passos: nenhum operador do script lê uma demanda ainda, e entram com eles.
8. **Stop dentro do C++** (MSPA, `circuit.pairwise`, Omniscape) e `ctx.stopped()` nos operadores longos.
   **Feito no Rust** (Unreleased): `ctx.progress` e o Stop entre os sorteios do `uncertainty.monte_carlo` (a cada 1%),
   os passos do `conn.metapopulation` e do `sdm.migrate` (`dispersal::migrate_watched`, o `migrate` de antes chama
   ele) e as distâncias do `conn.node_importance` (`node_importance_watched`); um teste em `stopping.rs` (contado e
   parado no meio). **O C++ espera o landschaft:** o MSPA inteiro, o `circuit.pairwise` por par e o Omniscape só
   param entre as chamadas (os blocos do MSPA em tiles, os pontos do Circuitscape); o pedido de um callback ou de
   uma flag de cancelamento olhada nos laços está no IDEAS.md de lá ("Cancelar as chamadas longas do C++",
   2026-11-01).
9. **Viewer:** o último elemento de um `control.each` e um passo escolhido de um laço.
   **Feito** (Unreleased): o `control.each` dá o que os nós do corpo deram para o último elemento (`node.port`, como
   o `control.repeat`), e `trace::shown_key` / `loop_on` levam o viewer até eles ("(the last element)"). Um passo
   escolhido de um laço do nível de cima: no cabeçalho do viewer, "step"/"element" com *last* e o número
   (`step_chooser`); `Session::choose_step` diz ao `Gate` (`want_step`) o passo cujo corpo a rodada manda
   (`Message::Body`; ao andar passo a passo, todos: o passo em que parou aparece); `Session::bodies` guarda, e o de
   um laço tirado do cache vem da rodada anterior. Um passo não guardado: *Run to see it* roda o laço de novo (fora
   do cache, a não ser congelado). Falta: laços dentro de grupos ou de outros laços (só os do nível de cima).
10. **Nó congelado:** o que só ele lê dentro de um grupo não roda mais.
   **Feito** (Unreleased): `Engine::only_for_frozen` acha, no corpo de um grupo, os nós lidos só por nós congelados
   com o resultado segurado (ou por nós assim, de trás para frente); eles não rodam. Os nós que as saídas do grupo
   leem rodam sempre, e um nó que nada lê também (um write, um view). Nos laços, nada muda. Teste: `first`, lido só
   por `second` congelado, não roda mais (`cache.rs`).
11. **QGIS:** o `.qgz` conferido no PyQGIS.
    **Feito** (Unreleased): `qgis::tests::a_project_for_pyqgis` (ignorado; escreve na pasta `TERRAFLUX_QGIS_DIR`)
    grava um `.qgz` com um mapa de classes, pontos rotulados como quadrados e feições em regras; `tools/check_qgz.py`
    o lê no Python do QGIS (`python-qgis-ltr.bat`, `QT_QPA_PLATFORM=offscreen`). As 17 conferências passam no QGIS
    3.34.14: o projeto lido, o CRS (EPSG:32651), as três camadas válidas, o mapa paletted com as classes 1, 2 e 3,
    os rótulos ligados pelo `name`, o marcador quadrado, o `RuleRenderer` com uma regra por classe e um `ELSE`, e um
    símbolo em cada regra.
12. **Trace:** a variável de iteração do CLUE.
    **Esperando o landschaft** (conferido em 2026-11-03, landschaft 1.56): o `CluesOut` do C++ só devolve a alocação,
    as iterações e as diferenças, não o vetor final das variáveis de iteração; o pedido está no IDEAS.md de lá ("As
    variáveis de iteração do CLUE", 2026-11-01). Quando vier: uma linha "why" a mais no cartão do `clue.step` (a
    aptidão mais a elasticidade mais a variável de iteração de cada classe na célula).
13. **Document:** tabelas dentro de tabelas.
    **Feito** (Unreleased): uma célula de uma tabela (`grid`) ou um valor de uma tabela por nome (`items`) pode ser
    uma lista, linhas ou registros por sua vez (`nested_ok`, até 4 níveis do JSON; mais fundo, a configuração
    segue como texto). Na tabela ele aparece como um botão com o resumo (`[3]`, `2×3`, `{2}`); aberto, é editado
    logo abaixo da tabela, com o nome da linha e da coluna (`nested_editors`, o mesmo `values` de novo, com os campos
    à parte), e a mudança volta para a célula. Assim ficam editáveis as zonas do `manual_targets`, os `demands` do
    `demand.regional` (uma linha por região) e a tabela do `demand.table`, no Document e no painel Node. Teste:
    `tables_inside_tables`.

## O Viewer como central de mapas, e as vistas no lugar do `view.plot`

Pedido de 2026-11-03. Hoje o Viewer é rápido (tiles feitos por threads, desenhados pela GPU), mas ver mais de um
resultado abre abas: o duplo clique num nó abre uma aba `Map` só dele **e** mostra o mesmo mapa no Viewer; cada
`view.plot` que roda abre outra aba; e sobrepor camadas só se faz encadeando nós `view.plot` pela entrada `under`.
Há duas formas de ver um resultado (clicar no nó, ou ligar um plot nele), e isso confunde.

O que se quer: o Viewer como **uma central de visualização**, como um SIG (várias camadas ao mesmo tempo, com
ordem, transparência, estilo, comparação), mantendo a velocidade de hoje, e **uma forma só de ver** um resultado.

### Decisões (combinadas em 2026-11-03)

- **Ver é clicar no nó.** O clique mostra o resultado como a **camada de prévia** (no topo, em itálico), que o
  próximo clique troca, como a aba de prévia do VS Code: clicar continua rápido e as camadas não se acumulam.
  **Fixar** (duplo clique, o 📌 da camada ou arrastar o nó até o mapa) a torna uma camada da lista. Nenhuma aba
  de mapa à parte.
- **O `view.plot` sai do grafo.** Uma figura não é um cálculo: o que o plot guarda (título, camada de baixo,
  opacidade, comparação, gráfico de uma tabela) é o que uma lista de camadas guarda. Ele vira uma **vista salva**
  (`[[views]]` no arquivo do modelo, fora dos nós), feita no próprio Viewer (*Save as view*), e é ela que o
  Document, o relatório, o QGIS, o script Python, a comparação de cenários e as figuras leem.
- **Os modelos antigos continuam abrindo e rodando:** os `view.plot` viram vistas ao abrir (migração), e o
  operador fica registrado como obsoleto (fora da biblioteca e das buscas) para o CLI e o MCP por um tempo.

### O que já existe e é aproveitado

- `map_view::MapView`: rasters empilhados com opacidade (`show_stacked`, `RasterLayer`), vetores por cima em
  tiles (`vector_tiles`), basemap (B), cortina (`show_compared`, `Curtain`), grades diferentes postas umas nas
  outras (`place_on`), o cursor de uma vista marcado na outra (`ghost`), a série do pixel clicado (`picked`).
- `builder::Shown`: o que uma aba de mapa mostra (nó, porta, banda, `overlays` de vetores, `rasters` por cima com
  opacidade, `compare`, `playing`): já é uma composição, espalhada hoje entre ele e a `Session` (`preview`,
  `preview_band`, `preview_overlays`).
- `builder::plot_stack` (a cadeia de `under` de um plot), `legend_view`, `palette` (a paleta lembrada por saída),
  `difference.rs`, `figure.rs` (a figura para artigo), `animation.rs`/`mp4.rs` (GIF e MP4), `qgis.rs`.
- O passo de um laço escolhido no cabeçalho do Viewer (`step_chooser`, 60b796c).

### Fases (um commit por fase; release só quando pedido)

1. **Duplo clique só no Viewer; plots sem abas.** (pequena)
   - `Action::Open` → `builder::open_map` → `Session::open_map`: em vez de uma aba `Pane::Map`, o nó vai para o
     Viewer (selecionado, roda até ele se preciso, o Viewer na frente). Até a fase 2 "fixar" é só mostrar.
   - Um `view.plot` que roda não abre mais aba (`poll`: `plots` → `open_map` + `quiet`); ele aparece no Viewer
     quando o nó é clicado, com a sua cadeia de `under`, como hoje.
   - Testes: duplo clique não abre `Pane::Map` e põe o Viewer na frente com o nó; um template que roda não abre abas
     `Map` (o teste de `canvas.rs` que esperava as abas dos plots muda).
2. **O painel de camadas.** (grande)
   - Estado: `Viewer { layers: Vec<Layer>, preview: Option<Layer> }` no lugar de `Shown` e dos `preview*` da
     `Session`. `Layer { from: node.port, kind (mapa, pilha, série, rasters em disco, vetores, tabela), visible,
     opacity, coloring/paleta, banda ou passo, nome }`. As camadas seguem os nós renomeados e saem com os
     removidos.
   - O painel à esquerda do mapa (recolhível): a prévia em itálico com 📌, depois as camadas fixas (a de cima por
     cima); caixa de visível, arrastar para a ordem, opacidade, paleta e legenda de cada uma, banda/passo, *Zoom to
     layer*, tirar. Atalhos: espaço esconde a de cima (como hoje), B o basemap.
   - Desenho: `show_stacked` com N rasters, cada um com os seus tiles (chave: nó, porta, banda, revisão, cores);
     camada escondida não pede tiles; a grade de referência é a da primeira camada raster visível (ou
     `s.reference`); as outras grades por `place_on`. Outro CRS: avisado e não desenhado nesta fase (fase 5).
   - **Identificar:** o cursor diz o valor de cada camada visível (e os atributos da feição de cada camada de
     vetores), no lugar do valor de uma só.
   - Uma tabela no Viewer é o gráfico de hoje (`charts.rs`), como uma camada à parte (não se sobrepõe a mapas).
   - Velocidade: medir com `canvas::timings` (release, `--ignored`) 1 e 5 camadas de 10 000 × 10 000; nenhum
     quadro mais lento com uma camada só do que hoje.
   - Testes: clicar troca a prévia; fixar mantém; ordem/opacidade/visível mudam o desenho (pixels do harness); o
     identificar lista as camadas; renomear e remover um nó.
3. **Comparar e dividir dentro do Viewer; o fim das abas `Map`.** (média)
   - Cortina entre a camada de cima e a de baixo (ou duas escolhidas); a diferença (`difference.rs`) como camada
     derivada.
   - Dividir em 1, 2 ou 4 vistas ligadas (zoom e lugar juntos, o cursor marcado nas outras), cada uma com a sua
     lista de camadas (ou a mesma com outra banda ou passo); tocar a série junto.
   - Saem `Pane::Map`, `map_tabs`, `beside_viewer`, `opened`/`quiet`/`raised` (`LAYOUT_VERSION` 12: um layout
     guardado com abas `Map` abre sem elas).
   - A figura (`figure.rs`) e o GIF/MP4 (`animation.rs`) partem das camadas e da divisão do Viewer.
4. **As vistas salvas no lugar do `view.plot`.** (a maior)
   - Formato: `[[views]]` no modelo (`terraflux_graph::Model::views`, serde, omitido quando vazio): `id`, `title`,
     `layers = [{ from = "node.port", opacity, palette, band, style }]`, `compare = { with, how }`,
     `chart = { kind, x, y, by }` para tabelas, extensão opcional, `note`/`why` (o registro). O `check` confere as
     referências (`views.<id>.layers[i]`); renomear e remover nós atualizam as vistas (`editor.rs`).
   - Migração ao ler (`terraflux_graph`): cada `view.plot` vira uma vista (`value`/`features`/`table` → camadas, a
     cadeia de `under` → as de baixo, como `plot_stack`; `opacity`, `compared`/`compare`, `chart`/`x`/`y`/`by`,
     `title`); o nó sai, e o que lia `plot.shown` passa a ler o que o plot mostrava. Salvo, o arquivo já tem
     `[[views]]`. O operador `view.plot` fica obsoleto (fora da biblioteca, `describe_operator` diz para usar as
     vistas) até a 2.0.
   - Quem lê os plots passa a ler as vistas: `document.rs` (as figuras de cada seção), `params_view::comparable`,
     `legend_view` (sem o salto pelo plot), `batch_view`, `report.rs`, `qgis.rs` (um grupo de camadas por vista, com
     os estilos), `python.rs`, `figure.rs`, `methods`, `advice.rs` (o conselho que sugeria um plot sugere uma vista),
     `op_notes`, `phrases` (a frase da vista no Document).
   - No app: *Save as view* no painel de camadas, a lista das vistas no Viewer e no Document; abrir uma vista carrega
     as suas camadas; *Update view* grava as mudanças.
   - Templates: os 101 `view.plot` de `templates.rs` viram vistas (pela própria migração, conferida à mão nos
     títulos). MCP: `set_view` e `remove_view` (`tools.rs`, `terraflux mcp`), o guia `results`, o assistente; a
     tradução de scripts Python cria vistas. Site: 134 menções em 47 tutoriais; `plots-and-tables.md` reescrito como
     "Views and figures".
   - Testes: cada template migrado dá as mesmas camadas que o `plot_stack` dava; um modelo antigo com `view.plot`
     roda no CLI; o Document, o relatório e o QGIS dos templates iguais antes e depois.
5. **Depois** (a decidir, cada um à parte): reprojetar outro CRS por tile, ao desenhar; medir distância e área;
   soltar um arquivo no Viewer (o `raster.read` criado e a camada fixada); a tabela de atributos de uma camada de
   vetores; rótulos; o histograma de cada camada; exportar a vista como PNG ou GeoTIFF composto.

### Em aberto

- O nome na interface: *Views* (o que o Viewer guarda) ou *Figures* (o que vai para o artigo e o relatório).
- Se uma comparação de cenários (`params_view`) passa a mostrar uma vista inteira (todas as camadas) por cenário.
- Por quanto tempo o `view.plot` obsoleto fica (proposta: até a 2.0).
- As fases 2 e 3 trocam `Shown` e o `preview` da `Session`: o trabalho no Viewer feito em paralelo (o passo de um
  laço, 60b796c) entra nelas, e um trabalho ainda aberto ali precisa terminar antes.

## Paridade com o landschaft na priorização, progresso e parada

Registro de 2026-10-16. O `prioritize.optimize` passou a resolver como o landschaft (o motor de corte e o de
fecho antes do HiGHS, o Gurobi quando instalado), com barra de progresso e Stop no meio da execução. Ficou para
depois:

1. **O resto do roteamento do landschaft** (`prioritize/_milp_solve.py`), para a mesma velocidade em todos os
   problemas:
   - o motor de corte com zonas (`solve_zone_cut`, `opt/zones_cut.cpp`): mínimos conjuntos com zonas e
     penalidades de borda ou conectividade, que hoje vão ao HiGHS;
   - a busca por subconjuntos depois do motor largo (`_wide_then_subset`, `improve_subset`): `max_features` e as
     filogenias;
   - comparar com o landschaft, nos mesmos dados, cada objetivo e cada tipo de unidade (tabelas, polígonos, H3).
2. **O Gurobi além do `prioritize.optimize`:** as re-soluções do `prioritize.evaluate` (custo de reposição e rank),
   o `prioritize.calibrate` (soluções em paralelo: uma thread do Gurobi em cada), o `restoration.plan` e o
   `prioritize.actions`.
3. **Parar no meio e dizer quanto falta em outros operadores longos:** hoje só os solvers param; um nó que não
   para (o `prioritize.evaluate`, o MSPA, os circuitos, o ajuste dos SDM, as métricas por blocos) é deixado
   terminar em segundo plano um segundo depois do Stop. Basta o operador chamar `ctx.stopped()` nos seus laços e
   `ctx.progress(...)`; no C++, perguntar `opt::stop_asked()` (ou um equivalente fora do `opt`).
4. **O submódulo do landschaft:** o `WatchScope` já está no `main` do landschaft (merge `db23ef9`); atualizar o
   submódulo para ele é a primeira etapa de "Integrar o landschaft 1.50.0", abaixo.
5. **Testes instáveis vistos:** `a_computer_lends_itself_to_another_by_its_code` (`crates/app/tests/ui.rs`) falha às
   vezes na suíte inteira e passa sozinho; `the_terrain_is_landschafts` (`crates/ops/tests/surface.rs`) difere em
   1e-16 no VRM só no build de debug (passa em release).
   `a_model_runs_and_every_tab_draws` (`crates/app/tests/ui.rs`, o `can_run()` logo depois de abrir o modelo) falha
   às vezes no `cargo test --workspace` e passa no programa `ui` sozinho.
   `settings_themes_and_favourites` (`crates/app/tests/ui.rs`) falha na suíte do `ui` com várias threads (as
   coleções das configurações são globais) e passa com `--test-threads 1`.

## Integrar o landschaft 1.50.0

Registro de 2026-10-16. O submódulo `extern/landschaft` está no `a302941` (a base `55d03a8`, de antes da 1.47.0, e
o `WatchScope`); o `main` do landschaft, com o merge `db23ef9`, está 43 commits à frente: a 1.50.0, cerca de 34 mil
linhas de C++ novas em `hydro`, `serv`, `swat`, `surf` e `sim`, e três bibliotecas novas no CMake (`hydrocore`,
`servcore`, `swatcore`). A ordem sugerida:

1. **Feito** (0.86.0). **Atualizar o submódulo para o `main`** e conferir o Terraflux com ele: o `build.rs` do `terraflux-ls-sys`
   ligando as bibliotecas novas (mesmo antes de haver operadores), a suíte inteira e os testes de paridade (os
   ótimos do prioritizr em release, `--include-ignored`), os pacotes dos três sistemas (o CI e o WSL). Nada em
   `src/cpp/opt` mudou desde a base além do `WatchScope`.
2. **Novos operadores, cada um bit a bit com o landschaft:**
   - **Feito** (0.86.0). **Hidrologia do terreno** (`hydro`): preenchimento e rompimento de depressões (menor custo), direções e
     acumulação de fluxo (D8 e o fluxo múltiplo do pygeoprocessing), traçado dos rios, bacias, HAND, TWI, SPI e STI.
   - **Feito** (0.86.0). **Serviços ecossistêmicos do InVEST** (`serv`): carbono, qualidade do habitat, produção anual de água e
     hidroeletricidade, a razão de entrega de sedimentos (SDR) e o índice de conectividade.
   - **Feito** (0.86.0). **Superfícies** (`surf`): os geomorfons (o `r.geomorphon` do GRASS), o topo de morro da APP (Lei 12.651/2012)
     e o potencial gravitacional do Dinamica (`CalcInteractionPotentialMap`).
   - **Feito** (0.86.0). **Simulação** (`sim`): o `ContinuousPatcher` (o `PatcherContinuousState` do Dinamica) e as estradas endógenas
     (o construtor de estradas do Dinamica).
   - **O SWAT+** (`swat`): o modelo de bacias portado do Fortran, bit a bit com o SWAT+ (projetos de rasters como o
     QSWAT+ e o SWAT+ Editor, o manejo das HRUs, a rotação de culturas, o MUSLE, a calibração, as saídas). O maior:
     pede um desenho próprio no builder (o projeto como um valor, as tabelas das saídas, os mapas das HRUs e dos
     trechos, `run_many` nos lotes).
     - **Feito** (0.86.0): rodar um projeto TxtInOut (`swat.run`: as tabelas impressas num passo e o clima).
     - Falta: o projeto a partir de rasters (o `watershed` do QSWAT+ e o `from_tables` do SWAT+ Editor, o clima
       com o gerador, `from_rasters`) como um valor do grafo, os mapas das HRUs e dos trechos (`hru_map`,
       `channel_map`, `reach_discharge`), alterar parâmetros do projeto num nó (para os lotes e a calibração) e a
       pasta TxtInOut enviada com o modelo nos lotes distribuídos.
3. **Tutoriais e templates** de cada um, como os do site do landschaft (a hidrologia do terreno, o SWAT+).
4. **O paralelismo do landschaft** (`on=` para as re-soluções da priorização, os SDM e o Monte Carlo): no
   Terraflux, os lotes distribuídos já cobrem esse uso; conferir se vale levar o `on=` às re-soluções de um nó só.

## Várias máquinas sem saber de computação

Registro de 2026-10-14. Os lotes em várias máquinas (`serve`, `worker`, `submit`, `calibrate`, o motor por tiles
com `--coordinator`) existem desde 0.4.0, mas só pela linha de comando: subir o coordenador, saber o IP, abrir a
porta, rodar um worker em cada máquina, sem autenticação nem TLS. A ideia é que o usuário veja **"Rodar em mais
computadores"** e **"Emprestar este computador"**, nunca "coordenador", "porta" ou "worker". A ordem:

1. **Feito** (0.74.0). **Lotes no aplicativo** (numa máquina): uma janela "Executar várias vezes" com réplicas, cenários (uma tabela
   em vez do TOML) e calibração (parâmetros com faixas, o mapa observado), sobre o `Local::start` que já existe;
   progresso, tempo restante, a tabela dos resultados e "aplicar os melhores ajustes ao modelo".
2. **Feito** (0.75.0). **Robustez** que o leigo não vê: a versão dos workers checada (hoje o `WorkerHello.version` é ignorado), o
   coordenador com os dados e os resultados em disco (hoje tudo na RAM, mensagens de até 1 GiB sem streaming),
   lotes retomáveis (ver e baixar os resultados depois, o aplicativo reabre o lote), a máquina sem suspender
   enquanto há tarefas, o worker que pausa quando o usuário volta ao computador; os arquivos lidos pelos nós
   (`raster.read`, `vector.read`, shapefiles com os arquivos irmãos, pastas e padrões) enviados com o modelo.
3. **Feito** (0.76.0). **Rede local sem configuração:** o coordenador embutido no aplicativo de quem lança o lote; "Emprestar este
   computador" nos outros; pareamento por um código curto (descoberta por broadcast UDP, o código como segredo:
   token de autenticação e TLS com o certificado fixado); a regra do firewall no instalador.
4. **Feito** (0.77.0). **Clusters (HPC) por SSH com um perfil salvo:** host, usuário, partição e tempo do Slurm, ou um servidor sem
   escalonador; o `submit --on <perfil>` (e o botão) confere o Terraflux do outro lado, sobe o coordenador e os
   workers, abre o túnel e traz os resultados.
5. **Feito** (0.78.0). **Motor por tiles sem armazenamento compartilhado** (avançado): os workers recebem os mapas pelo coordenador e
   guardam as camadas dos passos nele, em vez de caminhos iguais em todas as máquinas.
6. **Feito** (0.79.0). **Nuvem:** máquinas alugadas que viram workers com um script de inicialização (`cloud-init`) gerado pelo
   Terraflux, ligadas ao coordenador pelo código.

## Decisões pendentes de uma revisão

Registro de 2026-10-15. Ao escrever os tutoriais do site, vários problemas apareceram; os que tinham correção
clara entraram em 0.80.0. Estes ficaram para decidir:

1. **As classes de feições padrão do `sdm.fit` na validação cruzada.** Com `classes = "default"`, o maxnet escolhe
   as classes pelo número de presenças de cada ajuste (`l` abaixo de 10, `lq` abaixo de 15, `lqh` abaixo de 80,
   senão `lqph`), então as dobras (menos presenças) podem ter menos classes que o modelo final: as métricas
   descrevem outro modelo que o mapeado (ex.: 66 presenças por dobra, `lqh`; 83 no total, `lqph`). Resolver as
   classes pelo total antes das dobras é o certo, mas quebra a igualdade bit a bit com o landschaft, que faz o
   mesmo: mudar nos dois juntos (e nas referências dos testes). Por ora a descrição de `classes` avisa.
2. **O `change.fos` trata um FOS 0 como dado ausente.** `compute_fos_change` (landschaft, `cpp/gtb/change.cpp`)
   descarta `v == 0`, mas 0 é uma conectividade válida da floresta (FAC). Os ganhos e perdas diferem um pouco do
   `change.simple` (613 contra 615, 72.221 contra 72.258 no exemplo do site). Conferir com o código do GTB antes de
   mudar.
3. **O CLUE-S que não converge.** O algoritmo é o do `allocateclues.c` do lulcc: a variável de iteração anda
   `diferença × scale` células. Com o `scale` padrão (0.0005), em mapas de dezenas de milhares de células ela
   oscila e, sem convergir, fica a última iteração (às vezes o mapa inteiro de uma classe). O template usa
   `scale = 0.000005`. Opções: guardar a melhor iteração em vez da última, ou um passo relativo à demanda
   (afastando-se do lulcc), ou só avisar melhor.
4. **Comportamentos das ferramentas de referência** que apareceram nos exemplos: o corte do Biodiverse em `k`
   grupos com o dendrograma incompleto (`bio.cluster` com condição espacial e `bio.region_grower` não dão `k`
   grupos), o FUTURES com `incentive_power` acima de 1 (a demanda não é atendida e vira `overflow`), o MOLUSCE com
   regressão logística (9 de 320 mudanças pedidas no exemplo), o Marxan 9–13 % acima do ótimo provado. Conferir
   se são mesmo das ferramentas originais.
5. **As configurações de um submodelo como entradas.** Hoje uma configuração não pode ler uma entrada (`edge_width =
   "$x"`; o `check` acusa desde 0.80.0), então um submodelo não expõe as suas. Pede um desenho: as referências nas
   configurações entrando nas dependências, no cache, na expansão dos submodelos e nos lotes distribuídos.
   **Feito** (1.11.0), como o item 1 de "A linguagem do grafo".
6. **Regras do CLUE-S e a alocação ordenada** (vistas nos tutoriais `clue-s-rules` e `ordered-allocation`):
   células restritas ainda são convertidas pelas regras de vizinhança (com limiar 0,9 para a floresta, as 6.727
   células do bloco restrito mudaram); as regras de vizinhança também convertem as células que a classe já tem e
   não passam no limiar; a alocação ordenada estocástica não atende à demanda quando uma classe que cresce é
   servida depois das que encolhem (no Plum Island, todas as ordens menos 2,1,3, inclusive a 1,2,3 padrão do
   Wizard; a determinística atende sempre). Conferir com o código do lulcc antes de mudar.

## Terraflux Server: a interface no navegador, o motor no servidor

Registro de 2026-10-15. Um "RStudio Server do Terraflux": a interface no navegador com a fluidez do desktop
(60 fps), o motor (Rust, o C++ do landschaft, o GDAL, o HiGHS) nativo no servidor. Não exige reescrever o C++ nem
levar tudo para WASM: só a interface vai para o navegador.

**Por que fica fluido.** Quem desenha cada quadro é a GPU de quem usa (o egui em WASM, sobre WebGPU ou WebGL2): o
canvas dos nós, os painéis e o arrastar e o zoom dos mapas são locais. Do servidor vêm só dados novos (tiles que
faltam, resultados), como num mapa web: a rede atrasa a chegada dos dados, não os quadros. É o oposto de
transmitir a tela (noVNC, Kasm), em que cada quadro é vídeo.

**Arquitetura.** O `terraflux-app` depende hoje direto de `ops`, `exec`, `gdal` e `dist` (20 dos 26 arquivos):
separá-lo em
- `terraflux-client`: a interface egui (editor, painéis, viewer, Wizard, Many runs), para o navegador (WASM) e para
  o desktop;
- `terraflux-protocol`: o que trafega (o modelo em TOML, comandos, eventos de progresso, tiles, estatísticas);
- `terraflux-server`: sessões, execução, cache, serviço de tiles, arquivos.

Entre a interface e o motor, um trait `Backend` (rodar até um nó, cancelar, progresso, pedir tiles e estatísticas,
listar, enviar e baixar arquivos, abrir e salvar modelos, lançar lotes) com duas implementações: `Local` (o que o
app faz hoje, no mesmo processo: o desktop não muda) e `Remote` (por WebSocket, ou gRPC-web com o `tonic` que já
existe).

**Mapas sem perder fluidez.**
- Tiles com os valores (float32, ou inteiros para classes), comprimidos (zstd), não imagens; a pirâmide de níveis
  no servidor, com a lógica que o viewer já tem (só o que está à vista, na resolução da tela, o nível grosseiro
  enquanto o fino chega).
- As cores na GPU do cliente (rampas, legendas e opacidade num shader): trocar a paleta é instantâneo; o valor
  sob o cursor lido do tile local.
- O que é pesado no servidor (contagem das classes, histogramas, métricas), em números.
- Os mapas de fundo direto dos provedores pelo navegador, ou por um proxy no servidor quando o CORS não deixar.
- No navegador o WASM do egui tem uma thread só: o que o app faz hoje em threads (colorir, contar as classes) vai
  para shaders, Web Workers ou o servidor.

**O servidor.**
- `terraflux server --listen ...` no binário que já existe, ao lado de `app`, `serve` e `worker`: serve a interface
  WASM e a API; imagem Docker; TLS por um proxy (Caddy, nginx).
- Sessões como as do RStudio: um processo por usuário (uma falha não derruba os outros), a pasta de trabalho de
  cada um, limites de CPU e memória, fim por inatividade, modelo e layout lembrados entre visitas.
- Execução longa que segue com o navegador fechado (o mecanismo de retomar lotes já existe) e é reaberta depois.
- O Many runs pelo coordenador do `terraflux-dist` que já existe, com clusters (Slurm) e nuvem.
- Autenticação: no início um token ou o código de pareamento atual; depois o login da instituição (OIDC) pelo
  proxy.
- Um navegador dos arquivos do servidor no lugar do `rfd`, com envio e download.

**Windows, macOS e Linux.** O binário já é empacotado para os três com o GDAL, as peças novas (tokio, axum ou
tonic) também rodam nos três, e o servidor não precisa de GPU (o desenho é no navegador).

| Sistema | Uso mais natural | Como serviço | Vários usuários isolados |
|---|---|---|---|
| Linux | servidor do laboratório ou da instituição, nuvem, cluster | systemd, Docker | o melhor caso: um processo por usuário com a conta dele, limites por cgroups ou contêiner |
| Windows | a estação forte do grupo servindo os notebooks | Windows Service | funciona com um processo por sessão; rodar cada sessão como outro usuário do Windows dá mais trabalho: uso pessoal ou grupo pequeno de confiança |
| macOS | o Mac forte servindo o notebook ou o iPad | launchd | como no Windows: uso pessoal ou grupo pequeno |

**Escopo recomendado.**
- O modo pessoal ou de grupo nos três sistemas: um processo, autenticação por token ou pelo código de pareamento,
  a pasta de trabalho do próprio usuário. Cobre "a minha máquina forte, acessada do navegador de qualquer lugar".
- O modo multiusuário completo (contas separadas, cotas, Docker, login institucional) só no Linux, ao menos no
  início: é onde laboratórios e instituições hospedam esse tipo de serviço.

**Também: abrir este Terraflux no navegador.** Um botão no app desktop que serve a sessão aberta a outro aparelho
da rede (ou de fora, com a máquina alcançável) por um código, como o painel Computers. O uso principal é a própria
pessoa: o computador forte fica no laboratório com o modelo e os dados, e ela continua a mesma sessão do notebook
em casa ou numa reunião, pelo navegador; o trabalho é feito na máquina do laboratório. Difere do "Emprestar este
computador" de hoje, em que a máquina só roda tarefas dos lotes de outra pessoa, sem interface. Dar o código a um
colega para ele abrir a sessão também seria possível, mas é um uso secundário. Nos três sistemas, sem instalar
nada além do app.

**Pontos técnicos.** (1) Desacoplar o app do motor: o maior trabalho, verificável pelos testes de interface
(`egui_kittest`), o desktop sem mudar. (2) A thread única do navegador. (3) WebGPU nos navegadores modernos,
WebGL2 como reserva (o wgpu já faz). (4) Isolar usuários e caminhos de arquivos (os plugins WASM já rodam em
sandbox).

**Fases.**
1. A separação cliente e backend, com o `Local`: o desktop igual.
2. O cliente compilando para WASM: sem as dependências do motor, do GDAL, do `rfd` e das threads.
3. O servidor mínimo, um usuário: rodar modelos, tiles, arquivos, progresso. Com 1 a 3, um protótipo num servidor
   de laboratório.
4. Multiusuário: sessões por processo, autenticação, persistência, limites, Docker.
5. Lotes, clusters e nuvem pelo coordenador que já existe.

Hoje, sem mudar código, dá para chegar perto transmitindo o app do servidor (Xvfb e renderização por software,
noVNC, Xpra, Guacamole, KasmVNC, Selkies; vários usuários com JupyterHub e `jupyter-remote-desktop-proxy`, ou
Kasm Workspaces): funciona já, com todos os operadores, mas é vídeo da tela (latência, banda, menos nitidez).

## Terraflux no navegador, sem servidor (Lite), e todo em Rust

Registro de 2026-10-15. Discutido como possível, sem nada a fazer por ora.

- **Lite no site da documentação.** O motor compilado para WASM rodaria os exemplos dos tutoriais na própria
  página (um botão "Rodar no navegador" sob cada grafo, as configurações mexidas pelo leitor), hospedado no GitHub
  Pages: só arquivos estáticos. Uma thread (o Pages não deixa pôr os cabeçalhos COOP/COEP; há o contorno do
  `coi-serviceworker`), mapas pequenos; os templates desenham os próprios dados, sem arquivos a baixar.
- **O C++ pelo Emscripten.** O Rust tem o alvo `wasm32-unknown-emscripten`, em que o `emcc` liga tudo: o
  landschaft (os `#pragma omp` são ignorados sem `-fopenmp`, as chamadas `omp_*` estão protegidas por `_OPENMP`),
  os shims do `cxx`, o Eigen e o HiGHS num `.wasm` só. Pede: o CMake do `terraflux-ls-sys` com o `emcmake`,
  exceções do C++ no WASM (`-fwasm-exceptions`), um modo sem `rayon`, deixar de fora o GDAL, o `terraflux-dist` e
  a interface egui (o `wasm-bindgen` não roda nesse alvo: a ponte com o JavaScript por funções C exportadas,
  `ccall`/`cwrap`), e o sistema de arquivos em memória do Emscripten para as entradas e saídas. Mais rápido de ter
  que portar, mas frágil e maior (10 a 30 MB antes de comprimir).
- **Todo em Rust.** Portar as ~72 mil linhas de C++ do landschaft daria um build só com `cargo`, um paralelismo só
  (`rayon`), segurança de memória e o alvo `wasm32-unknown-unknown` (inclusive a interface egui no navegador);
  custa meses, a igualdade bit a bit é difícil (a ordem das somas, o `exp` e o `log` de cada sistema, o Eigen), o
  landschaft usa o mesmo C++ (o caminho limpo seria o núcleo em Rust e o landschaft por PyO3), e o HiGHS e o GDAL
  não têm equivalentes em Rust. Se for feito, aos poucos, kernel por kernel, com os testes de paridade.

## Atualização automática

Registro de 2026-10-16. O aplicativo avisa quando há uma versão nova e a instala com um clique, nunca sozinho.

**Como funcionaria.**
1. **Verificar:** ao abrir (no máximo uma vez por dia, numa thread), o app lê um manifesto pequeno
   (`latest.json`: a versão, o resumo do CHANGELOG, o endereço e o SHA-256 de cada pacote, uma assinatura) e, se
   houver versão nova, mostra um aviso discreto: "Terraflux 0.81.0 disponível · Ver novidades · Atualizar".
2. **Baixar e conferir:** o pacote baixado em segundo plano, o SHA-256 e a assinatura conferidos com uma chave
   pública embutida no binário (ed25519/minisign) antes de qualquer instalação: o app executa o que baixa.
3. **Instalar, por pacote:**
   - Windows `.msi`: o mais simples. O `installer/terraflux.wxs` já tem o `UpgradeCode` fixo e o `MajorUpgrade`:
     o app fecha e roda `msiexec /i terraflux-<versão>-x64.msi /passive` (pede administrador, a instalação é por
     máquina).
   - Windows `.zip` e Linux `.tar.gz`: o app troca o próprio executável e a pasta ao lado (no Windows, renomeando
     o `.exe` em uso; o crate `self-replace`).
   - macOS: o zip baixado, o app fecha e substitui `/Applications/Terraflux.app` (baixado pelo próprio app, o
     arquivo não ganha a marca de quarentena: a assinatura ad hoc não atrapalha).
   - Linux `.deb`: não pelo app (pediria root); um repositório apt (no GitHub Pages) e o `apt upgrade`; o app só
     avisa.
4. **Linha de comando:** `terraflux update`, para quem usa só o terminal; nos clusters, nada automático.

**Antes.**
- **Releases públicos:** hoje o repositório é privado (o site com `releases_public: false`) e o app não baixaria
  sem um token. Tornar os releases públicos, um repositório público só de binários, ou o manifesto e os pacotes
  no GitHub Pages.
- **Reprodutibilidade:** uma versão nova pode mudar resultados. Desligar a verificação nas Settings, "pular esta
  versão", as novidades sempre à vista, nada instalado sem pedir; a versão do Terraflux gravada nos metadados das
  saídas.
- **Lotes distribuídos:** coordenador e workers na mesma versão; o coordenador recusa um worker de outra versão
  dizendo qual instalar.
- **Tamanho:** os pacotes trazem o GDAL (dezenas de MB). Depois: baixar só o executável quando o GDAL não mudou.
- **Release:** o `latest.json` gerado e assinado pelo processo de release; a chave de assinatura fora do
  repositório.

**Fases.**
1. Verificar, avisar, baixar, conferir a assinatura e instalar o `.msi` (2 a 3 dias).
   **Feito** (1.19.0), conferido pelo `SHA256SUMS.txt` do release (sem assinatura ed25519); o `.msi` por usuário,
   sem administrador. **Corrigido** (1.21.0): o PowerShell lançado sem console (`DETACHED_PROCESS`) terminava
   sem rodar nada, e o app fechava sem instalar.
2. A troca no macOS e no zip/tar.gz, o repositório apt.
   **Feito** (1.21.0): o `Terraflux.app` (numa pasta que o usuário pode escrever) trocado pelo do zip, a pasta
   do `.tar.gz` pela nova, os pacotes `.deb`, `.rpm` e do Arch pelo gerenciador com o `pkexec`; a atualização que
   não instalou dita ao abrir de novo, com o log. Falta: o zip do Windows, o repositório apt, a assinatura.

## Ferramentas gerais de raster, vetor e análise

Registro de 2026-10-18. As ferramentas de todo dia de um SIG que faltavam entre os operadores (as de tabela entraram
em 1.5.0: `table.filter`, `calc`, `aggregate`, `join`, `sort`, `select`, `pivot`). Em blocos, nesta ordem:

1. **Estatística de rasters:** `raster.stats` (resumo de um mapa ou de cada camada: n, média, sd, mín, máx,
   percentis, histograma), `stack.reduce` (por célula sobre uma pilha: média, sd, mín, máx, mediana, soma, válidos,
   moda, a camada do máximo), zonal com zonas em raster (`raster.zonal`, com o mapa de cada célula recebendo a
   estatística da sua zona) e a área de cada classe por zona (`raster.tabulate`), `raster.correlation`
   (correlação e covariância entre mapas, a matriz de uma pilha), a correlação e a regressão local em janela móvel
   entre dois mapas, `raster.standardize` (z-score, mín-máx, percentis). **Feito** (1.6.0): os sete, o
   assunto Raster analysis da biblioteca e o template Raster statistics: summaries, zones and correlation.
2. **Vizinhança e limpeza de mapas de classes:** estatística focal geral (média, mediana, moda, sd, variedade,
   amplitude, percentil; janela quadrada, circular, anelar ou gaussiana), filtro de maioria e sieve (as manchas
   menores que N células, como o `gdal_sieve`), `raster.label` (as manchas como um raster de ids), crescer e
   encolher uma classe N células. **Feito** (1.6.0): `raster.focal`, `raster.majority`, `raster.sieve`,
   `raster.label`, `raster.expand` e `raster.shrink`, e o template Cleaning a map of classes.
3. **Vetor:** `vector.polygonize` (raster em polígonos), `vector.filter` e `vector.calc` (atributos),
   `vector.geometry` (área, perímetro, comprimento, centróide), overlay (interseção, união, diferença), join
   espacial (contar pontos por polígono, os atributos de quem contém), `vector.reproject`, `vector.from_table`
   (pontos das colunas x e y), pontos aleatórios ou regulares dentro de polígonos. **Feito** (1.6.0): os nove
   (`vector.overlay` e `vector.spatial_join` pelo GEOS do GDAL), o assunto Vector analysis e o template Vector
   tools: polygons, overlay and points.
4. **Análise espacial clássica:** densidade de kernel de pontos e contagem de pontos por célula, curvas de nível,
   viewshed, a área real de cada célula numa grade lat/lon, classificação por quebras (quantis, intervalos iguais,
   Jenks, desvio padrão). **Feito** (1.6.0): `vector.density` (kernels e contagem por célula),
   `raster.contour`, `raster.viewshed`, `raster.cell_area` e `raster.classify`, e o template Spatial analysis.

## O que ainda falta entre os operadores

Registro de 2026-10-17. Uma revisão dos 326 operadores (0.86.0) atrás do que quem trabalha com mudança de uso e
conservação espera encontrar e ainda não está no Terraflux. A ordem é a de importância; os mais baratos e que
destravam outros são o 1 e o 3.

1. **Estimativa de área e acurácia por amostragem** (Olofsson et al. 2014, as "boas práticas"). O `validate.*`
   compara mapa com mapa (kappa, quantidade e alocação, os três mapas), mas não há o fluxo que revisores e o IPCC
   pedem: o desenho da amostra estratificada (o tamanho e a alocação por estrato), a matriz de confusão dos pontos
   de referência, a área ajustada com o intervalo de confiança, as acurácias do usuário e do produtor com o erro
   padrão. Referências: o `mapaccuracy` do R, o AREA2 do Google Earth Engine.
   - **Feito** (0.87.0): `accuracy.sample_size` (a fórmula de Olofsson e as alocações: mínimo por estrato,
     proporcional, igual, Neyman), `accuracy.sample` (os pontos sorteados como o `default_rng(seed).choice` do
     numpy) e `accuracy.assess` (o `olofsson` e o `stehman2014` do mapaccuracy, a 1e-12; os intervalos de confiança),
     com o template Accuracy and area of a map.
2. **Emissões da mudança de uso e linhas de base de REDD+.** O `services.carbon` dá o estoque; faltam as emissões e
   remoções por transição ao longo do tempo (os fatores por par de classes, IPCC Tier 1 e 2, as curvas de rebrota da
   vegetação secundária, emissões comprometidas ou distribuídas no tempo) e a alocação do risco de desmatamento do
   Verra (VT0007/VM0048: o mapa de risco jurisdicional, o benchmark pela distância à borda, a alocação da
   atividade). Era um dos usos principais do Dinamica EGO no Brasil.
   - **Feito** (0.91.0): `carbon.emissions` (os estoques por classe e as curvas de rebrota, as emissões e remoções
     por período e transição) e o VT0007 (`redd.nrt`, `redd.vulnerability`, `redd.fit`, `redd.allocate`,
     `redd.assess`), iguais ao UDef-ARP da Clark Labs; o template REDD+ baseline (VT0007). Falta: as emissões
     comprometidas ou distribuídas no tempo (o decaimento da madeira morta e do solo).
3. **O LEAS do PLUS e o aprendizado de máquina em geral** (`ml.fit`, `ml.predict`). O `cars.step` recebe a
   probabilidade de crescimento pronta: falta o LEAS (a floresta aleatória sobre as expansões entre dois mapas), e
   sem ele o PLUS fica pela metade. Um `ml.*` genérico dá também o potencial de transição como o Land Change
   Modeler, a classificação supervisionada de imagens e modelos para qualquer tabela.
   - **Em Rust, sem Python.** O executável não leva Python (ver `docs/PLAN.md`), e o `ml.predict` precisa rodar no
     motor por tiles e nos workers dos lotes distribuídos, onde não haveria Python nem scikit-learn instalados. A
     floresta aleatória já está em Rust: `crates/ops/src/sdm/forest.rs` é o `RandomForestClassifier` do
     scikit-learn 1.9 portado bit a bit (o construtor em profundidade, o `rand_r`, a introsort, o MT19937 do
     numpy). O scikit-learn fica como a referência dos testes de paridade, gerada à parte e guardada no
     repositório, como as do landschaft. Crates como `linfa` e `smartcore` não dariam os mesmos bits.
   - **Os passos:** (1) tirar a floresta do `sdm` para um módulo `ml` comum, o `sdm.fit` usando-o; (2) várias
     classes (hoje os nós guardam `[f64; 2]`: o Gini com `n` classes, como o do scikit-learn) e regressão
     (`RandomForestRegressor`, o critério do erro quadrático); (3) a importância das variáveis (a de impureza e a
     de permutação) e a dependência parcial; (4) o `HistGradientBoostingClassifier`/`Regressor`, que costuma
     ganhar da floresta em tabelas: o binning, os histogramas por variável (a soma de cada um em ordem fixa, então
     dá para igualar os bits), gradientes em float32 e o `exp` do numpy (o `npmath` já tem o de float32); (5) o
     LEAS e o potencial de transição sobre o `ml`.
   - **Feito** (0.88.0): os passos 1, 2 e 5, e a importância de impureza do 3: `ml.fit`, `ml.predict` e
     `ml.potential` (o `Estimator` do landschaft: presença ou expansão, o LEAS), bit a bit com o scikit-learn 1.9 e o
     landschaft; o modelo como um valor (`ml_model`, gravado em JSON); as árvores crescidas e as células previstas
     em paralelo; o template PLUS (LEAS and CARS). Falta: a importância por permutação e a dependência parcial
     (passo 3), o `HistGradientBoosting` (passo 4) e a previsão no motor por tiles.
   - **Feito** (0.89.0): o passo 4, o `HistGradientBoostingClassifier`/`Regressor` bit a bit com o scikit-learn 1.9
     (sem early stopping), em `ml.fit` e `ml.potential` (`method = "gradient_boosting"`).
   - **Feito** (0.90.0): o resto do passo 3, a importância por permutação (`ml.importance`) e a dependência
     parcial (`ml.partial_dependence`, o método brute), os números do scikit-learn.
   - **O modelo como um valor do grafo** (um tipo novo, gravável e relido de um arquivo), para ajustar uma vez numa
     amostra das células e prever em mapas do tamanho do mundo: a previsão é célula a célula, então entra no motor
     por tiles sem mudança, com as árvores achatadas em vetores e as faixas em paralelo (`rayon`).
   - **A rede neural (MLP) do Land Change Modeler** fica por último: o `MLPClassifier` do scikit-learn multiplica
     matrizes pelo BLAS, cuja ordem das somas muda com o processador, então só daria igualdade dentro de uma
     tolerância. Em tabelas, a floresta e o boosting costumam ser melhores.
4. **Dados de entrada com um clique** (`data.download`). Há o `sdm.download_gbif`, mas não as variáveis: o WorldClim
   e o CHELSA (atuais e futuros do CMIP6), o MapBiomas (coleções por bioma, estado ou município), o Copernicus DEM,
   o SoilGrids e catálogos STAC (o Planetary Computer), recortados para a área e a grade do modelo. Atende ao
   princípio da preparação guiada para quem tem pouca experiência; o painel Prepare poderia oferecê-los.
   - **Feito** (0.92.0): `data.download` com o WorldClim (atual e CMIP6), o CHELSA (atual e CMIP6), o Copernicus
     DEM, o SoilGrids, o MapBiomas (coleções 9, 10 e 11) e qualquer URL, só a janela da grade lida dos servidores.
     Falta: os catálogos STAC (o Planetary Computer pede assinar os endereços) e oferecê-los no painel Prepare.
5. **O Código Florestal completo.** Há o topo de morro (`surface.hilltop`); faltam as APPs dos rios pela largura (os
   rios do `hydro.streams`), as nascentes, as declividades acima de 45°, as bordas de tabuleiro, e o déficit e o
   excedente de Reserva Legal por imóvel do CAR (o SICAR como entrada), como nos trabalhos do Soares-Filho.
   - **Feito** (0.93.0): `forest_code.app` (as APPs do art. 4º e as faixas do art. 61-A nas áreas consolidadas) e
     `forest_code.reserve` (o balanço da Reserva Legal: arts. 12, 15 e 67), com o template Forest Code: APP and
     Reserva Legal. Falta: as bordas de tabuleiros e chapadas (a linha de ruptura do relevo), o limite do
     art. 61-B (10 % e 20 % do imóvel) e a compensação da Reserva Legal entre imóveis (art. 66).
6. **Análise de sensibilidade global.** O "Executar várias vezes" tem réplicas, cenários e calibração; faltam os
   desenhos de Morris e de Sobol (as amostras de Saltelli) e os índices (efeitos elementares; primeira ordem e
   totais com intervalos por bootstrap), sobre os lotes e a distribuição que já existem. Referência: o SALib.
   - **Feito** (0.94.0): `sensitivity.design` e `sensitivity.analyze` (Morris e Sobol, iguais ao SALib 1.6; a
     sequência de Sobol embaralhada do scipy) e o comando `terraflux sensitivity`, que roda o desenho nos lotes
     (aqui, em clusters ou na nuvem) e mede a saída nos workers. Falta: a aba no "Executar várias vezes" do
     aplicativo.
7. **Estatística espacial e interpolação.** Nenhum operador interpola: IDW e krigagem ordinária (o variograma
   ajustado) para estações de clima e amostras de solo. Nem mede a autocorrelação local: o I de Moran global, o
   LISA e o Gi* de Getis-Ord (os hotspots de desmatamento), referências no PySAL (`esda`). E a tendência célula a
   célula de uma série de mapas (Mann-Kendall e a inclinação de Sen), muito usada com o NDVI.
   - **Feito** (0.95.0): `stats.trend` (Mann-Kendall e a inclinação de Sen, iguais ao pymannkendall),
     `stats.moran`, `stats.lisa` e `stats.getis_ord` (iguais ao esda, contiguidade rook ou queen) e o modelo
     "Trend and hot spots".
   - **Feito** (0.96.0): `stats.variogram` (o variograma amostral e o ajuste do gstat, esférico, exponencial ou
     gaussiano), `stats.kriging` (krigagem ordinária com a variância) e `stats.idw`, iguais ao gstat, e o
     modelo "Interpolation: IDW and kriging".
8. **O resto do InVEST:** a razão de entrega de nutrientes (NDR), a polinização e a produção sazonal de água, os
   mais pedidos depois dos quatro que já estão. Entram primeiro no landschaft, como os outros.
   - **Decidido** (2026-10-18): primeiro no landschaft (`ls.services`, o C++ em `src/cpp/serv`, bit a bit com o
     natcap.invest), depois aqui pelo `terraflux-ls-sys`, como os quatro de `services.rs`. Registrado no
     IDEAS do landschaft ("Do Terraflux para o landschaft", item 4).
   - **Feito** (1.21.0): `services.ndr`, `services.pollination`, `services.seasonal_water_yield`.
   - **A fazer** (landschaft 1.55.0, 2026-10-27): `services.coastal_vulnerability` (o C++ em
     `src/cpp/serv/coastal.cpp`: os 16 raios de fetch por ponto da costa contra a terra, a profundidade ao longo
     deles, as janelas de raster; o resto em Python/shapely em `services/_coastal.py`, a portar; referências
     `tests/data/reference/invest/coastal_{full,required,lonlat}.npz/json`). E os serviços com entradas em grades
     diferentes, alinhadas e cortadas às bacias ou à AOI como o InVEST (`services/_common.py` `align_inputs`, o warp
     do landschaft com o transformador aproximado do GDAL); a correção do SWY (recarga sem dado = sem recarga) já
     vale aqui, conferir com as referências novas.
9. **Dispersão de espécies no tempo.** O SDM de vários períodos com a dispersão célula a célula (como o MigClim) ou
   com uma metapopulação (o modelo de função de incidência), juntando o `sdm.*` e o `conn.*` num laço.
   - **Feito** (0.97.0): `sdm.migrate`, o autômato do MigClim 1.6 (o mesmo resultado para uma semente: o
     `rand()` da CRT portado) e o modelo "Species migration (MigClim)".
   - **Feito** (0.98.0): `conn.metapopulation`, o modelo de função de incidência de Hanski como o `spom` do
     MetaLandSim 2.0 (o mesmo resultado para uma semente, o `runif` do R) e o modelo "Metapopulation
     (incidence function model)". Falta juntar os dois num laço (a ocupação das manchas pelo SDM de cada
     período).

Fora dos operadores:
- **O nó de script em Python**, previsto no plano como componente opcional e isolado (um Python embutido), nunca no
  laço da simulação.
- **Animações das simulações:** os mapas coletados num `control.repeat` exportados em GIF ou MP4, com a legenda e o
  passo. **Feito** o GIF (Unreleased; ver "Linha do tempo das simulações" acima); falta o MP4.

## Rios como rede (do hydrographr)

Registro de 2026-10-19. Uma leitura do [hydrographr](https://glowabio.github.io/hydrographr/) (o pacote do R do
Hydrography90m) atrás do que serviria ao Terraflux. A família `hydro` já vai do DEM à bacia (`fill`, `breach`,
`flow_direction`, `flow_accumulation` com pesos, `streams`, `stream_links` com o trecho a jusante e a ordem de
Strahler, `watersheds`), e o recorte, a junção, a reclassificação, o nodata e a estatística zonal já existem. O que
falta é tratar a rede como um **grafo de trechos**, a partir da tabela do `stream_links`. Em ordem de valor:

1. **Variáveis a montante por trecho ou ponto** (o `get_upstream_variable`): a média, a soma ou a proporção de cada
   classe na bacia a montante de cada trecho ("% de floresta a montante", "área urbana a montante"). É o cruzamento
   do uso do solo com a água, o núcleo do Terraflux. Hoje sai em parte do `flow_accumulation` com pesos dividido
   pela área; falta o operador que dê a tabela por trecho e as proporções das classes.
   - **Feito** (1.8.0): `hydro.upstream`, por trecho do `stream_links` ou por ponto: as estatísticas de mapas
     (contagem, soma, média, mínimo, máximo, desvio padrão) e a proporção de cada classe na terra a montante, a
     área, o trecho ou o ponto abaixo; o mapa de uma coluna pintado nos trechos. O template "Rivers: the land
     upstream and the gauges' catchments".
2. **Pontos na rede e a bacia de cada ponto** (o `snap_to_network`, por distância ou por um fluxo acumulado mínimo,
   e o `get_upstream_catchment`): estações fluviométricas, barragens, ocorrências de peixes. O `stream_links` já
   ajusta os exutórios por dentro; faltam um `hydro.snap` à parte e um `hydro.upstream_catchment` que dê a bacia de
   cada ponto em polígonos.
   - **Feito** (1.8.0): `hydro.snap` (pela distância, ou o rio mais próximo com um fluxo acumulado mínimo) e
     `hydro.upstream_catchment` (um polígono por ponto, as bacias de pontos um acima do outro sobrepostas, e o mapa
     das bacias encaixadas entre um ponto e o seguinte).
3. **Conectividade dos rios:** a distância ao longo da rede entre pontos (`get_distance`), os vizinhos de um trecho,
   as faixas a montante e a jusante (`get_buffer_along_the_network`), a centralidade (`get_centrality`) e o
   preenchimento das lacunas entre trechos de habitat (`fill_habitat_gaps`). Encaixa na família `conn`: um
   `conn.graph_from_streams` sobre a tabela dos trechos dá a centralidade e a importância dos nós que já existem.
   Indo além do hydrographr, o **DCI** (Dendritic Connectivity Index, Cote et al. 2009) com as barragens como
   barreiras, e com o `prioritize.*`, quais barragens remover: o diferencial mais forte.
   - **Feito** (1.9.0): `hydro.network_distance` (as distâncias pela rede entre todos os pares de pontos, a reta e se
     um drena para o outro), `hydro.along_network` (os trechos a montante e a jusante de cada ponto até uma
     distância), `hydro.centrality` (grau, magnitude de Shreve, o comprimento a montante, a intermediação e a
     proximidade do igraph no grafo não dirigido dos trechos) e `hydro.dci` (o DCI potamódromo e diádromo, simétrico
     ou assimétrico, o índice de alcance de cada segmento e o ganho de remover cada barreira, o `d_index`), iguais
     ao riverconn 0.3 (`tools/make_river_dci_reference.py`); o template "Rivers: dams and their connectivity (DCI)".
     Falta: o `fill_habitat_gaps` e a escolha de um conjunto de barragens a remover sob um orçamento (hoje, uma por
     vez).
4. **O Hydrography90m e o Environment90m no `data.download`:** a rede, as sub-bacias e as variáveis ambientais por
   sub-bacia do mundo inteiro a 90 m, sem derivar do DEM, como as fontes que já estão lá. Conferir antes se o
   servidor deixa o GDAL ler só a janela da grade (os tiles de 20° são grandes).
   - **Feito** (1.10.0): o servidor do IGB (um Nextcloud) atende pedidos por faixas (HTTP 206), e os tiles são
     GeoTIFF em blocos de 256: o `data.download` com `source = "hydrography90m"` lê só a janela de cada uma das
     variáveis em tiles (sub-bacias, segmentos, bacias, direções, acumulação, ordens, declividades, distâncias,
     índices de fluxo) dos tiles sobre a grade. O Environment90m vem em tabelas por sub-bacia (zips por tile e
     variável), não em rasters: o `data.environment90m` baixa uma vez para o cache as tabelas das variáveis pedidas
     (clima observado e projetado do CHELSA, cobertura do ESA CCI por ano, SoilGrids, MERIT DEM, CGIAR, a própria
     hidrografia) nos tiles do mapa de sub-bacias e dá a tabela das sub-bacias do mapa e o mapa de uma coluna.
5. **SDM de água doce** (`get_modelfit_table`, `get_predict_table`): o `sdm.*` com a sub-bacia como unidade em vez
   da célula, com as variáveis a montante do item 1. Sai quase de graça depois dos itens 1 e 2.

Fora: as sub-bacias de Pfafstetter, os lagos (as interseções com a rede e as bacias dos lagos), os ids dos tiles e
das unidades regionais (internos ao Hydrography90m) e as funções `api_*` (o servidor deles). Por onde começar: os
itens 1 e 2 (pequenos, sobre o `stream_links`), depois o 3 com o DCI, e um template "Rios: uso do solo a montante e
barragens". **Decidido** (2026-10-21): direto em Rust (`crates/ops/src/rivers.rs`), sobre as direções D8 do
landschaft; no landschaft entram depois, já em Rust e não em C++ (registrado no IDEAS dele, item 11 de "Do Terraflux
para o landschaft").

## A linguagem do grafo: o que falta além dos operadores

Registro de 2026-10-21. A biblioteca de operadores já é ampla; a linguagem que os liga ainda é simples: o
`control.repeat` (com `carry`, `collect` e `until`), o `control.group`, o `control.submodel`, o `raster.calc`, o
`batch.run` (um operador sobre vários arquivos), o `plugin.wasm` (célula a célula) e um nó de um mapa rodado uma vez
por camada de uma pilha (×N no canvas). Basta para um modelo linear; para os fluxos de trabalho reais faltam peças.
Os itens 1 a 3 juntos fazem do grafo uma linguagem completa (sequência, repetição, iteração sobre coleções,
escolha, abstração) sem código.

**Na lógica dos nós:**

1. **Configurações que leem valores** (`edge_width = "$x"`; a decisão pendente 5 de "Decisões pendentes de uma
   revisão"). Trava o resto: um submodelo não expõe as suas configurações (não serve como operador reutilizável), um
   laço não varia uma configuração a cada passo (um limiar que muda com o ano), a calibração e os parâmetros só
   alcançam configurações fixas, e um valor não alimenta várias configurações ao mesmo tempo. As referências nas
   configurações entram nas dependências, no cache, na expansão dos submodelos e nos lotes distribuídos.
   - **Feito** (1.11.0): uma configuração lê `"$input"`, `"@step"`/`"@carried"` (um laço, um grupo) ou
     `{ from = "node.port" }` (um número, verdadeiro ou falso, um texto de outro nó; a forma explícita porque um
     texto como `"a.b"` continua texto). A referência entra nas dependências (o nó lido roda antes), no cache (o
     fingerprint), no `check` (o tipo conferido: mapas e tabelas recusados), na expansão dos submodelos (o `$nome`
     de uma configuração vira a entrada do grupo: um submodelo expõe as suas configurações), no editor (renomear,
     copiar, agrupar e desagrupar e remover a seguem) e no motor por tiles (só as entradas de valores). No
     aplicativo, o clique direito no nome de uma configuração lê um valor do modelo, e a linha mostra o que ela lê
     (`← $x`). Os lotes distribuídos recebem o modelo inteiro e o mesmo executor, sem mudança.
2. **Um laço "para cada"** (`control.each`): um corpo inteiro de nós para cada elemento de uma coleção (cada arquivo
   de uma pasta, camada de uma pilha, linha de uma tabela, polígono ou região, espécie), os resultados coletados numa
   pilha ou numa tabela. O caso mais comum: 30 espécies, 50 municípios, 38 anos de MapBiomas. Pede um tipo de
   coleção genérico (o `Series` hoje é só os passos de um laço).
   - **Feito** (1.12.0): o `control.each` vai sobre o que já é coleção, sem um tipo novo: as camadas de uma pilha
     (uma pasta, as bandas de um arquivo), os itens de uma série, as linhas de uma tabela (cada uma uma tabela de uma
     linha), as feições de um vetor (cada uma sozinha) e os valores de uma lista. Dentro, `@item`, `@index` e
     `@name`, e as outras entradas como as de um grupo; o que coleta é juntado como o de um nó rodado por camada:
     mapas numa pilha com os nomes, tabelas numa só com a coluna `item`, o resto numa série. Sem `carry` (cada
     elemento sozinho: isso é o `control.repeat`). Uma entrada de valor não dada passa a valer o valor do modelo
     também no executor (como o CLI e o app já faziam). O template "For each year: one analysis per map".
3. **Condicional** (`control.switch`): um ramo escolhido por um parâmetro (`method = "maxent" | "rf" | "glm"`), só ele
   rodado. Com os parâmetros e cenários, a escolha do método vira um cenário comparável (no Document e no Compare).
   - **Feito** (1.13.0): o `control.switch`, um grupo por ramo no corpo; roda o de id igual ao caso (`case_<n>` para
     um número, `true`/`false`, `otherwise` para o resto), as saídas as do ramo (todos dão as mesmas, conferido no
     `check`) e `branch`, o id do que rodou. O template "Choose the method: a switch and its scenarios".
4. **Valores escalares e textos** (`value.calc`): números que não são mapas e textos montados, com a sintaxe do
   `raster.calc` e interpolação de texto: o caminho `out/{species}_{step}.tif` num laço, um número de uma tabela que
   alimenta uma configuração.
   - **Feito** (1.13.0): o `value.calc`, `expression` com o avaliador do `raster.calc` numa célula (as entradas
     números, verdadeiro e falso como 1 e 0, uma tabela como `t.coluna` da linha `row`) e `text` com `{nome}` e
     `{value}`. Um número inteiro chega inteiro às configurações de inteiros.
5. **Submodelos como operadores de verdade:** com o item 1, um submodelo na biblioteca com descrição, portas
   tipadas, configurações e versão; bibliotecas de submodelos (do usuário, do grupo, compartilhadas). A comunidade
   estende a ferramenta sem escrever Rust.
6. **Plugins além da célula:** uma segunda interface do `plugin.wasm` que receba o mapa inteiro ou uma janela de
   vizinhança, dê várias saídas e trabalhe com tabelas. Opcional: um nó `script.python`/`script.r` para quem os tem
   instalados, marcado como não portátil para os lotes distribuídos (vai contra o "sem Python", mas é a saída de
   emergência que todo usuário avançado procura).
7. **Depuração do grafo:** desativar ou contornar um nó (bypass) sem apagá-lo; congelar o resultado de um nó caro
   entre sessões; tempo e memória por nó (o perfil da execução); um laço passo a passo, como um breakpoint (o Trace
   já mostra os valores, falta controlar a execução); um nó `check.assert` que falha quando uma condição não vale (a
   demanda atendida, a soma das áreas).
   - **Feito** (1.14.0): o bypass (`bypass = true` no nó: cada saída a primeira entrada do mesmo tipo; B no canvas,
     a caixa apagada), o tempo e a memória de cada nó (`Results.timings`, o corpo de um laço somado nos passos;
     "last run" no painel do nó, `terraflux run --profile`) e o `check.assert` (a expressão do `raster.calc` em
     toda célula com valor, ou do `value.calc`; `warn` só avisa). Falta: congelar o resultado de um nó entre
     sessões (o cache em disco) e o laço passo a passo.
8. **Testes de modelos** (`terraflux test`): asserções e saídas esperadas guardadas com o modelo, um template ou um
   modelo publicado conferido a cada versão nova do Terraflux.
   - **Feito** (1.15.0): `[[tests]]` no formato (`from`, `expected`, `tolerance`) e `terraflux test` (modelos ou
     pastas; os `check.assert` e os testes; `--update` grava os esperados, rasters de reais em float64).

**Fora do grafo:**

- **Reprodutibilidade:** um manifesto gravado com as saídas (a versão do Terraflux, os hashes dos dados de entrada,
  a semente, os parâmetros, o cenário) e um `terraflux run --reproduce manifest.json`. O cache em disco já existe;
  falta o registro.
  - **Feito** (1.15.0): o `manifest.json` de todo `terraflux run` (a versão, a plataforma, o modelo e os arquivos de
    entrada com o SHA-256, os valores, o cenário e os `--set`, as sementes, as saídas com o SHA-256) e o
    `terraflux reproduce manifest.json` (roda de novo e diz se cada saída é a mesma, byte a byte; os vetores pelas
    feições, porque o GeoPackage grava a hora). Falta: o manifesto das execuções do aplicativo.
- **Execução incremental entre sessões pela linha de comando**, como o `targets` do R: só o que mudou rodado de
  novo, com o cache persistente.
- **Integração com o ecossistema:** um provedor de Processing para o QGIS (os modelos do Terraflux como algoritmos do
  QGIS; provavelmente o maior multiplicador de adoção) e uma API Python para montar e rodar grafos, além do caminho
  do landschaft.
- **Dados em nuvem:** STAC e o Planetary Computer (já na lista do `data.download`, item 4 de "O que ainda falta entre
  os operadores").
- **A interface em português e espanhol:** com o Código Florestal e o MapBiomas entre os operadores, o público
  latino-americano é óbvio, e o aplicativo não tem i18n.
- **Datas como tipo:** séries com datas reais, não só passos numerados (clima, MapBiomas, séries de mudança).
- **Atualização automática:** desenhada em "Atualização automática".

**A ordem:** (1) as configurações que leem valores, que destravam os itens 2 a 5; (2) o `control.each` com um tipo
de coleção; (3) o `control.switch` e o `value.calc` com interpolação de texto, pequenos depois dos dois primeiros;
(4) o bypass, o congelamento e o perfil por nó, muito ganho no dia a dia com pouco custo; (5) o manifesto de
reprodutibilidade e o `terraflux test`.

## A calculadora de rasters: uma linguagem própria para mapas e pilhas

Registro de 2026-10-26. O `raster.calc` já tem o essencial da álgebra de mapas (aritmética, comparações, `if`, as
camadas de uma pilha por nome, as vizinhanças, o `null`), mas é uma expressão de uma linha numa configuração, um
avaliador lento e um mapa só na saída. A ideia: um nó de calculadora com uma linguagem própria, próxima do Python,
um console como o do Assistant, e o código salvo como um nó do usuário. A linguagem nova é um superconjunto da do
`raster.calc`: toda expressão de hoje é um script de uma linha válido, e os modelos, templates e tutoriais que o usam
(669 ocorrências em 163 arquivos) continuam funcionando. O `value.calc`, o `check.assert`, o `uncertainty.delta` e o
Trace usam o mesmo parser e ganham junto.

**A linguagem (decidida):**

- **Próxima do Python.** `=` atribui, `==` compara; `and`, `or`, `not` (e `&&`, `||`, `!` como hoje);
  `a if c else b` (e o `if c then a else b` de hoje); comparações encadeadas (`5 <= img < 8`); `**` para potência
  (o `^` continua potência, não o XOR do Python); `#` para comentários. Sem `&` e `|`: no numpy,
  `img >= 5 & img < 8` é avaliado na ordem errada.
- **Variáveis e regras.** Linhas em ordem, cada uma uma atribuição. A regra condicional é a do numpy:
  `alvo[condição] = valor`; uma linha depois sobrescreve a de antes, e a célula que nenhuma condição pega fica com o
  valor que já tinha. Uma entrada atribuída dentro do nó é uma cópia local.
- **`out`** expõe o que sai do nó, cada um uma porta do tipo do valor (mapa, pilha ou número): `out x = ...` ou
  `out x` de uma variável que já existe.
- **Bandas por ponto.** `s.elevation`; o nome que não é identificador corrigido (`1997` → `s._1997`,
  `NDVI-2020` → `s.NDVI_2020`), a banda sem nome `b1`, `b2`, ... O autocomplete mostra o nome usado e o original.
  Hoje `land.1997` é lido como `land` e o número `0.1997`.
- **Pilhas.** `s * 2` multiplica todos os pixels de todas as camadas e dá uma pilha com os mesmos nomes; pilha com
  mapa, o mapa aplicado a cada camada; pilha com pilha, camada a camada (o mesmo número de camadas, senão erro).
- **Reduções por célula** (só pilhas): `mean(s) sd(s) median(s) percentile(s, p) min(s) max(s) sum(s)`, um mapa com
  o valor de cada célula entre as camadas. Num mapa só, erro que aponta para o método. `min(a, b)` entre mapas como
  hoje.
- **Estatísticas globais** (métodos, um número): `img.mean()`, `img.sd()`, `img.min()`, `img.max()`,
  `img.percentile(90)`; numa pilha, todas as células de todas as camadas. Os parênteses separam o método da banda.
  Um número sai do nó como outra porta qualquer (`out p90 = img.percentile(90)`) e alimenta as configurações e os
  nós que leem números.

```python
# entradas: img (pilha do Sentinel-2), land (mapa de uso)
ndvi = (img.nir - img.red) / (img.nir + img.red)
out ndvi
out forest = land == 1 and ndvi > ndvi.percentile(25)
out cleaned = land
cleaned[land == 9] = None
out mean_ndvi = ndvi.mean()
```

**Fase 1: o motor.** O avaliador de hoje copia cada entrada que lê (`to_vec`), aloca um raster inteiro por nó da
árvore, calcula os três ramos de um `if`, roda numa thread, e o `nbmin`/`nbmax` custam O(r²) por célula. Em lugar
dele: a expressão compilada em bytecode e avaliada em blocos de alguns milhares de células (os intermediários no
cache), os blocos em paralelo no pool do motor, constantes dobradas e subexpressões comuns calculadas uma vez. As
vizinhanças e as estatísticas globais são barreiras (o halo; duas passadas), e o `nbmin`/`nbmax` por van
Herk/Gil-Werman. Com isso, a leitura por faixas com halo (`block_rows`) para rasters maiores que a memória. Nada muda
para o usuário; um benchmark contra o avaliador de hoje e os mesmos testes.
- **Feito** (Unreleased): a expressão compilada (`Compiler`, `Program` em `algebra.rs`) em blocos de 2048 células
  em paralelo, as constantes dobradas, as subexpressões comuns e as vizinhanças repetidas calculadas uma vez, os
  registradores reusados, os mapas lidos no lugar; o `nbmin`/`nbmax` separável por van Herk/Gil-Werman, o
  `nbsum`/`nbmean`/`nbcount` com as tabelas de somas em paralelo. O avaliador antigo ficou nos testes como
  referência: 660 expressões aleatórias dão o mesmo resultado bit a bit. Em 9 milhões de células e 4 threads: as
  expressões locais 5 a 20 vezes mais rápidas, `nbmax(a, 5)` 18 vezes, `nbmin(b, 25)` 400 vezes (45 s para
  0,11 s), `nbmean(a == 1, 2)` só 1,8 vez (a passada das colunas das tabelas de somas ainda numa thread). Falta: a
  leitura por faixas com halo e o `if` que não calcula o ramo que nenhuma célula do bloco usa.

**Fase 2: a linguagem.** O que está acima, sobre o motor da fase 1. Os nomes das bandas conhecidos antes de rodar: do
cabeçalho dos arquivos e, para uma pilha calculada, do esquema propagado pelo grafo ou do resultado em cache.
- **Feito** (Unreleased): a linguagem no `raster.calc` (o parâmetro `expression` é o script; `parse_script`, `Script`,
  `Stmt` em `algebra.rs`): as linhas, `=`, `alvo[condição] = valor`, `s.camada = valor`, `out`, as pilhas camada a
  camada, as reduções por célula, os métodos globais (somas por partes fixas, o mesmo resultado bit a bit com
  qualquer número de threads), `a if c else b`, as comparações encadeadas, `**`, `True`/`False`/`None`, `log` e os
  nomes das bandas (`band_names`). O script inteiro compila num programa só (as subexpressões comuns entre as
  linhas, todas as saídas numa passada). As portas de saída vêm do `out`: `OpSignature::outputs_from` e
  `declared_outputs` no grafo (o `check`, o canvas e o Document as leem), do tipo "qualquer valor" até a fase 3. O
  campo do script no painel do nó tem várias linhas. Decidido: as reduções e os métodos deixam de fora os valores
  que faltam (como as vizinhanças); o `min`/`max` de vários valores continua dando `null` quando um falta; o
  desvio-padrão é o amostral (n − 1); o percentil é o linear entre os dois vizinhos (o padrão do numpy e do R).
  Falta: os nomes das bandas antes de rodar (vai com o autocomplete, na fase 3). **Feito** na fase 3, para as
  entradas: do resultado da última execução, ou do cabeçalho do raster lido quando ele ainda não rodou.

**Fase 3: o console.** Um painel embaixo, como o do Assistant: o script com cores e o erro sublinhado onde está, o
autocomplete das entradas, das bandas, das funções e dos métodos, a prévia ao vivo na parte do mapa à vista em
resolução reduzida, e um modo REPL (`img.mean()` dá o número na hora, sem entrar no nó).
- **Feito** (Unreleased): o painel Calculator (`calc_view.rs`), aberto embaixo do modelo (View, os comandos, ou
  "Open in the calculator" no painel do nó), edita o script do `raster.calc` selecionado. O script com cores e os
  números das linhas; o primeiro erro sublinhado onde está e dito embaixo, com a linha (os da linguagem, os nomes
  que não são entradas nem foram dados antes, com "did you mean", e as camadas que uma pilha não tem); o
  autocomplete ao digitar ou com ctrl+space (as entradas com o que são, os nomes dados no script, as funções e os
  métodos com a sua ajuda, e depois de `s.` as camadas com o nome original); a prévia de cada saída ao lado,
  calculada numa thread 300 ms depois de parar de digitar, numa amostra de até 160 mil células do mapa inteiro (os
  raios das vizinhanças na escala da amostra; um raster em disco lido já amostrado); e embaixo um console que
  calcula uma linha no escopo do script, em todas as células (um número, ou o resumo e a imagem de um mapa ou de
  uma pilha), com `help` e `clear`. O script vai para o nó com ctrl+enter (que roda o modelo até ele), Apply, ou
  ao sair do editor. A parte do motor fica em `algebra_tools.rs` (`parts`, `problem`, `complete`, `preview`,
  `evaluate_line`, `describe`), testada sem a interface; o painel tem um teste de ponta a ponta com o
  `egui_kittest`. Falta: a prévia só da parte do mapa à vista (hoje é o mapa inteiro em resolução reduzida); os
  tipos das portas de `out` (ainda "qualquer valor"); as camadas de uma pilha calculada no script
  (`s2 = s * 2`, então `s2.`) no autocomplete; e o autocomplete no campo do painel do nó.

**Fase 4: o código salvo como um nó do usuário.** O item 5 de "A linguagem do grafo": "salvar grupo como nó" para
qualquer grupo, a calculadora o caso mais comum. Uma biblioteca do usuário com descrição, portas tipadas e o que
cada entrada exige (mapa ou pilha, as bandas pedidas, conferidas ao conectar). A versão gravada no modelo
(`meu/ndvi@3`) e uma cópia embutida: editar o nó não muda em silêncio o resultado de um modelo antigo, e o modelo
enviado a alguém roda sem a biblioteca.
- **Feito** (Unreleased): `terraflux_graph::user_node` (`UserNode`, `Uses`, `Library`, `update`): a biblioteca é
  uma pasta de arquivos `nome@versão.toml` (`nodes` nos dados do aplicativo, ou `TERRAFLUX_NODES`), cada versão
  gravada uma vez só (salvar o mesmo de novo não cria versão). No modelo, o nó é um `control.group` com `uses` (o
  nome, a versão, as entradas tipadas com as bandas pedidas e o valor próprio, as saídas tipadas) e a cópia do
  corpo daquela versão: roda em qualquer lugar, e uma versão nova só entra quando o nó é atualizado (o painel do nó
  oferece, mantendo as entradas dadas que a versão ainda tem; recusado se quebraria quem lê uma saída que sumiu).
  O `check` usa os tipos e o que é obrigatório (uma entrada não dada vale o valor próprio, no motor também); o
  motor confere as bandas antes de rodar ("ndvi@3 reads img's layers: it has no layer nir (it has b8, red)"), e o
  aplicativo avisa ao conectar. No aplicativo (`my_nodes.rs`): "Save as my node..." no painel de um grupo ou de um
  `raster.calc` (a calculadora vira um grupo de um nó, as entradas `@nome`, as saídas as do `out`), o diálogo com o
  que foi inferido para mudar (os tipos pelo que o nó recebe e pelo que deu na última execução, as bandas pelo
  `algebra_tools::layers_read` dos scripts, os valores literais como valor próprio), a seção My nodes no painel
  Operators (a busca também), os comandos na paleta, e o painel de um nó do usuário (a versão, o que cada entrada
  pede e o que falta, "Update to", "changed here since", "Make it a plain group"). Falta: bibliotecas do grupo e
  compartilhadas (o `meu/` do nome), as configurações de um nó do usuário como configurações (hoje são entradas
  com valor próprio), a busca rápida (`/`) com os nós do usuário, o MCP e o assistente (fase 5).

**Fase 5: a IA.** O Assistant escreve o script de uma calculadora a partir de um pedido: o parser confere, os nomes
e as bandas contra as entradas reais, uma prévia, e a mudança passa pelo painel Changes. Também explica um script e
salva um nó do usuário.
- **Feito** (1.21.0): as ferramentas `write_calc` e `explain_calc` (no app e no `terraflux mcp`, também entre as
  poucas dos modelos pequenos). O `write_calc` confere o script antes de mudar o modelo (a linguagem, os nomes
  contra as entradas do nó, as camadas das pilhas) e o roda numa amostra das células (os valores da última
  execução, ou os rasters lidos dos arquivos; no MCP, o modelo rodado até o que ele lê): recusado com o motivo, ou
  escrito com o que cada saída dá (as classes e as suas partes, ou o menor, a média e o maior). O `explain_calc` dá
  cada entrada e o que ela é, as camadas lidas e o que cada nome guarda na amostra. No painel Calculator, um campo
  pede o script ao assistente (Write) e Explain pede a explicação; a mudança vai ao painel Changes como as outras.
  Falta: salvar um nó do usuário pelo assistente (vai com o item 10 de "A interface: o que falta nas partes já
  feitas").

**Fase 6: o site.** O tutorial de álgebra de mapas reescrito com a linguagem nova; os templates só onde ela melhora.

## Lotes e laços: processar uma pasta tem de ser o mais fácil do Terraflux

Registro de 2026-10-30. Pedido do usuário: "acho que temos que melhorar muito a forma de fazer batch.run e loops
dentro do terraflux. Acho que todo node tipo um node de mspa, por exemplo, deve ter opção de fazer batch.run ao invés
de ter que chamar um node específico de batch.run [...] Além disso, acho bem estranho os nós que começam com "$" pois
vc não interage com eles, não pode mover, apagar...nada. [...] Quero que a opção de processar muitos dados como uma
pasta de rasters para um mspa, por exemplo, seja o mais fácil e intuitivo possível. Hoje em dia isso é a pior parte
do terraflux."

**O diagnóstico.** Há três formas de repetir, que não conversam:

- o `batch.run`: paralelo, retoma de onde parou, um arquivo por entrada, mas um operador só, configurado por texto
  (`params = {...}`), e o MSPA vira uma configuração de outro nó, não um nó;
- o `control.each`: um corpo de nós, mas um elemento por vez, tudo na memória;
- o ×N: um nó que recebe muitas camadas onde toma um mapa roda uma vez por camada. É o comportamento certo, e já
  vale para uma pasta (`raster.read` de uma pasta dá os arquivos como camadas, lidos só quando preciso). Mas cada nó
  roda sozinho e guarda o resultado de todos os arquivos na memória antes do próximo, e numa pasta de arquivos de
  grades diferentes (bacias, municípios, tiles) os mapas viram uma série sem nomes: o `raster.write` com `{name}`
  grava `10_mspa.tif` em vez de `clc_r0c0_mspa.tif`. Além disso, os operadores que tomam um raster (os
  `metrics.*`, que leem por blocos) não rodam ×N: o `metrics.class` de uma pasta mede só o primeiro arquivo, sem
  aviso.

**A ideia: uma pasta é um dado, e qualquer nó roda sobre ela.** Sem uma opção de lote em cada nó: quem decide é o
dado que chega. Ligar uma pasta a um nó que toma um mapa o faz rodar ×N.

1. **A pasta direto no nó.** O `raster.read` de uma pasta ou de um padrão (`*.tif`) é a coleção, lida arquivo por
   arquivo. No clique direito de um nó, **"Run on a folder..."** escolhe a pasta e põe o `raster.read` ligado à
   primeira entrada de mapa. As entradas de raster das métricas (os `metrics.*`, que leem por blocos e tomam um
   mapa só) também rodam ×N.
2. **Cadeias inteiras, arquivo por arquivo.** Pasta → reclassificar → MSPA → métricas: o motor roda a cadeia ×N como
   um pipeline por arquivo (lê, passa pela cadeia toda, grava, solta), e não nó por nó com todos os arquivos na
   memória. Os nomes dos arquivos seguem pela cadeia. Um arquivo que falha não para os outros: o aviso diz quais e
   por quê.
3. **A saída sem esforço.** Um `raster.write` na cadeia grava um arquivo por entrada com o nome dela (`{name}`; sem
   `{name}`, o nome da entrada antes do do arquivo: `mspa.tif` → `clc_r0c0_mspa.tif`). As tabelas viram uma só, com
   a coluna da entrada (o `table.write` dela grava um CSV só).
4. **Paralelo e retomável.** Vários arquivos ao mesmo tempo (os workers, no máximo as threads do motor); o que já foi
   feito pulado na execução seguinte (como o `batch.run`: os arquivos gravados e o registro `.json` de cada entrada,
   com as configurações, o tempo e a versão).
5. **Retorno enquanto roda.** "Testar no 1º arquivo" (ctrl+enter roda a cadeia só nele e mostra no Viewer); o
   progresso no nó (37/200); a lista das falhas com o motivo e "rodar de novo só as que falharam".
6. **Várias pastas pareadas pelo nome** (`uso/bacia_07.tif` com `estradas/bacia_07.tif`), não pela posição; as que
   ficaram sem par ditas.
7. **"Processar uma pasta" na tela inicial:** a pasta, o operador, as configurações e a pasta de saída, e o modelo
   montado (editável).
8. **O `batch.run` convertido** nessa forma ao abrir um modelo antigo (o operador como nó, a pasta como
   `raster.read`, a saída como `raster.write`), e o tutorial "Batch processing" reescrito. O `control.each` fica para
   o que pede um corpo explícito (as linhas de uma tabela, as feições de um vetor).

**Os nós "$" (e "@") viram nós de verdade.** Hoje as entradas do modelo são caixas presas numa coluna à esquerda do
canvas (`editor_view.rs`, montadas de `model.inputs`): não se movem, não se apagam, não se duplicam. A entrada do
modelo passa a ser um nó comum (`raster.read`, a pasta, um valor) com a opção "é entrada do modelo: nome ___"; quem
roda o modelo pelo CLI, pelo Many runs ou pelo Server só troca o caminho. O mesmo para os valores de um contêiner
(`@step`, `@item`, as entradas de um grupo): um nó "Input" no corpo, que se move, como o *Group Input* do Blender. Os
modelos antigos abrem convertidos (um `$x` vira esse nó).

**Falta decidir:** as tabelas só têm números, então a coluna da entrada numa tabela juntada é o número do fim do
nome (o ano de `forest_2010`) ou a posição (1, 2...), não o nome do arquivo. Texto nas tabelas é uma mudança grande
(o `Table` inteiro); por ora o registro `.json` e o relatório dão o nome de cada número. **Resolvido** sem texto
nas tabelas: `Table::labels` dá nomes aos números de uma coluna (a posição 1 é o primeiro nome), e a coluna `layer`
mostra e grava o nome do arquivo. Colunas de texto de verdade (vindas de um CSV, por exemplo) seguem por fazer.

**A ordem:** (fase 1) a cadeia por arquivo, os nomes, o `raster.write` na cadeia, as falhas que não param, as
entradas de raster ×N e o "Run on a folder..." (itens 1 a 3); (fase 2) o paralelo, o retomável, o registro e o
retorno enquanto roda (itens 4 e 5); (fase 3) os nós de entrada no lugar dos `$` e `@`; (fase 4) o pareamento por
nome, o "Processar uma pasta", a conversão do `batch.run` e o tutorial (itens 6 a 8).
- **Feito** (Unreleased), fase 1: no executor, `Engine::chain` acha a cadeia quando um nó recebe os arquivos de uma
  pasta (rasters em disco de vários arquivos, sem banda escolhida) onde toma um mapa: ele e os nós depois dele que
  tomam os seus mapas, com tudo o mais que leem já dado, e os `raster.write` dos seus mapas que gravam um arquivo
  por arquivo (`{name}` no caminho, ou arquivos de grades diferentes). O `run_chain` roda a cadeia arquivo por
  arquivo; o que dão é juntado como o ×N junta. Um arquivo que falha não para os outros (um aviso diz quais e por
  quê; erro só quando todos falham), e um aviso de alguns arquivos é dito uma vez, com eles. No nível de cima, os
  mapas só gravados em arquivos não ficam na memória quando passariam de 2 GB, e esses nós não entram no cache. Os
  mapas de grades diferentes juntados numa série levam o nome (o item `terraflux_layer` da grade, nunca gravado),
  então o `{name}` dá `clc_r0c0` e não `10`; o `raster.write` desses mapas sem `{name}` grava um arquivo cada.
  As entradas de raster que tomam um mapa só (`PortSpec::one`, nos `metrics.*`) rodam ×N. No aplicativo, "Run on a
  folder..." no painel do nó e no menu novo do clique direito num nó do canvas (`editor::run_on_folder`). Ficou de
  fora: o `raster.terrain`, o `distance.proximity` e o `landscape.mspa_tiled`, que dão rasters em disco, não
  rodam ×N (juntar rasters em disco por camada pede outra forma).
- **Feito** (Unreleased), fase 2: os arquivos em paralelo (`workers` do `raster.read`, por padrão as threads; num
  pool de rayon de tantas threads, que os operadores dividem; os resultados, as falhas e os avisos os mesmos, na
  ordem dos arquivos), o progresso no nó ("37 of 200 files"), o registro `.json` de cada arquivo ao lado do
  primeiro arquivo gravado (o arquivo lido, os gravados, os nós e as configurações, o tempo, a versão e um
  SHA-256 do que ele depende) e o retomável: quando só os `raster.write` da cadeia leem o que ela dá, um arquivo
  feito antes com os mesmos nós, configurações e arquivo (e os seus arquivos lá) não roda de novo. Isso dá também
  o "rodar de novo só as que falharam". O `first` do `raster.read` roda a cadeia nos primeiros arquivos. Falta: a
  lista das falhas num painel do aplicativo (hoje é o aviso, no painel Problems).
- **Feito** (Unreleased), fase 3, de outra forma que a escrita acima: as entradas continuam no formato
  (`inputs`, `$nome`), porque o CLI (`--set`), os cenários, os parâmetros, o Many runs, o Server, o MCP e os
  templates dependem dele. O que mudou foi o canvas: as caixas do que um escopo lê (as entradas do modelo, os
  valores de um contêiner, os nós de fora que um corpo lê) se movem, são selecionadas e apagadas como os nós, e o
  lugar fica no modelo (`Model::places`, pela chave `escopo|referência`). Uma entrada do modelo clicada é editada
  no painel do nó (renomear, com as referências, os parâmetros e os cenários seguindo; o arquivo ou o valor; a
  nota; apagar, desconectando quem a lia). "Make it a node" faz da entrada de um arquivo um `raster.read` dele
  (que pode então ler uma pasta), e "Make it an input of the model", no menu de um `raster.read` do nível de
  cima, o contrário (um mapa quando ele escolhe uma banda ou o arquivo tem uma, senão uma pilha). Falta, se
  fizer falta: um nó "Input" de verdade dentro dos contêineres (hoje é a caixa que se move).
- **Feito** (Unreleased), fase 4: um nó que recebe duas pastas (ou mais) roda nos arquivos pareados pelo nome
  (`paired` no executor): os nomes iguais, ou, quando isso pareia menos, os nomes sem o começo e o fim comuns de
  cada pasta, cortados num `_`, `-`, `.` ou espaço (`uso_bacia_01` com `estradas_bacia_01`); os sem par ficam de
  fora, ditos num aviso; o registro e o retomável cobrem os arquivos pareados. "Process a folder..." no menu File,
  na paleta de comandos e no canvas vazio: a pasta, depois o operador (a busca rápida, só os que tomam um mapa), e o
  modelo montado (`editor::process_folder`: `raster.read` da pasta, o operador, `raster.write` do primeiro mapa em
  `<pasta>_<op>/{name}_<op>.tif` e `table.write` da primeira tabela em `<pasta>_<op>.csv`), o operador
  selecionado para escolher as configurações. "Make it a chain of nodes" no painel de um `batch.run`
  (`editor::batch_to_chain`), explícito e não ao abrir, porque um `batch.run` cuja tabela ou relatório alguém lê
  não tem equivalente (é recusado, dizendo quem lê); também recusados uma lista de arquivos e o `recursive`. O
  tutorial "Batch processing" reescrito na forma nova (rodado nos 16 tiles do CORINE), e a seção de lotes de
  "Large maps" também. Falta: a lista das falhas num painel (fase 2) e, com texto nas tabelas, o nome do arquivo
  na coluna `layer`.
- **Feito** (Unreleased), o que faltava: as tabelas nomeiam os números de uma coluna (`Table::labels`, como um
  fator do R; o resto do `Table` continua de números), e a coluna `layer` de uma tabela juntada dos arquivos de uma
  pasta dá os nomes deles (o CSV, o `table.write`, as saídas, o visualizador e os gráficos mostram os nomes; nomes
  que terminam em números diferentes, os anos, continuam números). O registro de cada arquivo guarda o que é lido
  depois da cadeia quando é pequeno (uma tabela, um número, um texto), então o MSPA com as métricas também é
  retomável. A lista das falhas no painel do primeiro nó da cadeia (`Observer::failures`), com "Run again" e
  "Copy the list". O `distance.proximity` e o `landscape.mspa_tiled` dados uma pasta rodam em cada arquivo, como o
  `raster.terrain` e o `raster.fill` (que já faziam isso sozinhos: não precisam rodar ×N pela cadeia); antes mediam
  só o primeiro arquivo.

## O MCP e o assistente mais inteligentes: guias, conselhos de método, prompts

Registro de 2026-11-01. A pergunta: dá para o `terraflux mcp` dar à LLM recomendações e instruções mais precisas?
Dá, e o ganho não está em escrever mais instruções, e sim em *onde* a orientação chega. Instruções longas no
`initialize` se diluem, e os clientes as cortam: o Claude Code cortou as do Terraflux por volta de 4.000
caracteres (eram 5.535; a parte dos containers chegava pela metade). Uma recomendação dentro da resposta da
ferramenta, no momento em que o problema aparece, é seguida. O AI Assistant do app (`chat.rs`) chama as mesmas
funções de `crates/app/src/tools.rs`, então quase tudo abaixo serve aos dois; só os prompts, os resources e as
instruções curtas são do MCP (o app tem as suas, `APP_INSTRUCTIONS`, que não são cortadas).

Os níveis, do mais ao menos efetivo:

1. **Instruções curtas, o resto sob demanda.** **Feito** (Unreleased): `tools::INSTRUCTIONS` tem 2.164 caracteres
   (o fluxo, as regras de ouro e os nomes dos guias); o detalhe foi para `crates/app/src/guide.rs` (`GUIDES`), lido
   pela ferramenta `guide` (no MCP e no app) e pelos resources `terraflux://guide/<nome>`. Guias de construção
   (`nodes`, `calc`, `containers`, `record`, `results`: o que antes ficava nas instruções) e de método, passo a passo
   com os operadores e templates de cada passo: `land-change` (analisar, calibrar no 1º período, simular, validar
   numa data não usada na calibração e contra o modelo nulo, várias seeds), `connectivity`, `landscape-metrics`,
   `conservation-planning`, `species-distribution`, `data-preparation`. Testes: todo `familia.nome` citado num
   guia é um operador do registro, todo template citado existe, e as instruções ficam abaixo de 3.200 caracteres e
   citam todos os guias (`mcp::tests::the_instructions_are_short_and_name_the_guides`).
2. **Conselhos de método no `check_model` e depois do `run_model`.** **Feito** (Unreleased):
   `crates/app/src/advice.rs` (`advise(model expandido, registro, pasta, entradas carregadas)`), o que um revisor do
   artigo perguntaria, cada conselho com o que fazer e o guia (`advice: nó (op): ... (guide land-change)`), os
   `warning` antes. As regras:
   - simulação de mudança (`dinamica.step`, `lulcc.step`, `cars.step`, `clumondo.step`, `molusce.step`,
     `futures.simulation`, `crafty.allocate`) sem nenhum `validate.*`;
   - um `validate.*` cujo mapa observado (`observed`, `t1`, `reference`) é o mesmo `t1` de um `markov.fit` ou
     `woe.fit` (warning: validado no mapa da calibração);
   - um simulador com seed (ou `rng`) e nenhum `control.each`, `batch.run`, `uncertainty.monte_carlo`,
     `sensitivity.*` nem cenários: uma rodada é um futuro possível;
   - mapas em graus (as entradas carregadas e o cabeçalho dos arquivos dos `raster.read`, `GeoTags::is_geographic`
     ou o WKT pelo GDAL) com operadores que medem distância ou área (`metrics.`, `fragmentation.`, `landscape.`,
     `distance.`, `cost.`, `conn.`, `circuit.`, `restoration.`) e sem `raster.reproject` (warning);
   - `metrics.*`, `fragmentation.*` ou `landscape.*` lendo uma entrada `$x` com valores não inteiros (warning);
   - `sdm.fit` com `sdm.random_folds` e nenhuma dobra espacial.
   O `run_model` (MCP e o `ran` do app) mostra até 3 ("On the method (check_model tells it all)"). Teste: nenhum
   dos 54 templates recebe um warning (`advice::tests::the_templates_are_not_warned`). O `inspect_file` diz quando
   um raster está em graus ou sem sistema de coordenadas (`tools::grid_notes`). O `new_model` com template diz
   quais nós sorteiam dados (`nlm.*`) e o próximo passo (`tools::template_next`).
3. **O `describe_operator` com conselhos, método e vizinhos.** **Feito** (Unreleased): `crates/app/src/op_notes.rs`.
   Os vizinhos aprendidos dos templates (os operadores cujas saídas ele lê e os que leem as dele, sem `view.plot`
   nem `nlm.*`, e os templates que o usam; calculado uma vez, `OnceLock`), o método e as citações
   (`methods::method_of`, `Reference::cite`) e dicas escritas à mão para 19 operadores (`TIPS`: calibrações,
   simulações e validações de LUCC, MSPA, métricas, dobras de SDM, priorização, circuitos, custos). Teste: toda
   dica é de um operador do registro.
4. **Busca por objetivo, em português.** **Feito** (Unreleased): `crates/app/src/search.rs`: as palavras sem
   acento e sem as palavras pequenas (pt e en), cada uma achada por ela mesma, pelo singular (`-s`, `-es`,
   `-ões`/`-ães` → `-ão`) ou pelos sinônimos (`SYNONYMS`, ~110 termos do campo: *desmatamento* → deforest,
   *manchas* → patch, *pesos de evidência* → weights evidence); primeiro o que tem todas as palavras, senão os que
   têm mais ("No operator has all of ...; those that have the most"). No `list_operators` e no `list_templates`
   (que ganhou `query`; `tools::find_templates`).
5. **Prompts do MCP.** **Feito** (Unreleased): `build_model` (goal, data, model), `adapt_template` (template,
   data, model), `review_model` (model), `explain_model` (model, language), além do `explain_cell`. Cada um é um
   roteiro (ler o guia, achar o template, `inspect_file`, uma grade, `describe_operator` antes de usar, nota e
   why, `check_model` e os conselhos, rodar, agrupar por etapa, perguntar o que os dados não dizem). No Claude Code
   são comandos `/mcp__terraflux__review_model`.
6. **Resources do MCP.** **Feito** (Unreleased): `resources/list` (os guias e os arquivos dos templates,
   `terraflux://template/<slug>`), `resources/templates/list` (`terraflux://operator/{name}`, o
   `describe_operator`) e `resources/read` (-32002 quando não existe). No Claude Code, anexados com `@`.
7. **Avaliar o efeito.** **Feito** (Unreleased): duas tarefas a mais no `eval.rs` (10 agora): métricas pedidas em
   português (*densidade de bordas*) e o método de um modelo Dinamica revisado sem mudá-lo (a resposta tem de dizer
   que falta a validação). Falta **rodar** a avaliação antes e depois com o Qwen da HU (VPN) e um modelo grande,
   e guardar a comparação: é o que diz se os guias e os conselhos ajudam ou só alongam as respostas.

Falta:

- **Elicitation** (MCP 2025-06-18): o servidor perguntar ao usuário no meio de uma ferramenta ("qual classe é a
  floresta?", "qual o período da calibração?"). Exige que o servidor mande um pedido ao cliente e espere a resposta
  pela mesma entrada padrão: hoje `serve()` lê uma mensagem por linha e responde; teria de guardar os pedidos em
  aberto (um `HashMap<id, Sender<Value>>`, como os `runs` do `run_model`) e entregar a resposta do cliente à thread
  que espera. Só quando o cliente declarar `elicitation` no `initialize`; sem ele, a ferramenta segue pedindo à LLM
  que pergunte. Candidatos: `new_model` com template (as classes do mapa do usuário), `set_input` de um mapa de
  classes sem legenda.
- **Mais regras de método** (`advice.rs`), cada uma com teste e sem warning nos templates: o `markov.fit` com
  `steps` incoerente com o intervalo das datas (precisa das datas: um parâmetro ou o nome dos arquivos); drivers de
  `woe.fit` sem nenhuma distância (o caso mais comum dos modelos de desmatamento); resistência com zeros ou
  negativos lida por `circuit.*` ou `cost.transition` (dos dados carregados); `metrics.*` com mapas de classes
  demais (provável mapa contínuo em inteiros); priorização sem `prioritize.evaluate`; SDM sem limpeza nem
  thinning dos registros (`sdm.thin`, `sdm.one_per_cell`, ou `sdm.prepare`).
- **Os conselhos no app**: hoje chegam ao assistente; mostrá-los também no painel Problems (com o nível "advice",
  abaixo dos warnings de dados) e no relatório, na seção de checagens.
- **Mais dicas por operador**: os 19 de hoje cobrem o caminho do LUCC e alguns de padrões; faltam os de
  conectividade em grafo (`conn.*`: a distância de dispersão), hidrologia, serviços (InVEST), agentes, REDD+.
  Melhor ainda se as dicas morarem no próprio operador (um campo `advice` em `OpSignature`), como a documentação, e
  não numa tabela à parte em `op_notes.rs`.
- **`guide` para os modelos pequenos**: o modo de poucas ferramentas (`tools::FEW`) não tem o `guide`; ver na
  avaliação se acrescentá-lo ajuda o Qwen ou só aumenta o pedido.
- **Saída estruturada** (`outputSchema` e `structuredContent`, MCP 2025-06-18) no `check_model` (erros, warnings e
  conselhos como lista, com o nó e o guia) e no `run_model` (os arquivos escritos), para clientes que mostram ou
  usam os campos; o texto continua para os outros.
- **Guias em português**: os textos dos guias estão em inglês (como as instruções e os operadores); um
  `guide(topic, language: pt)` a partir de `pt.rs` para o assistente do app, que responde na língua do usuário.
