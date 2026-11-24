# Ideias para o Terraflux

Ideias levantadas no desenvolvimento que ficaram fora do escopo atual, para implementar em outro momento. O
plano e o andamento estão em [`docs/PLAN.md`](docs/PLAN.md). Uma ideia concluída é marcada como **Feito**
(versão).

## Próximas partes do landschaft no builder

Registro de 2026-10-11. O MSPA já está no builder (`landscape.mspa`, bit a bit com o GTB), com os modelos
neutros (`nlm.*`) para testar fluxos sem dados. A ordem sugerida:

1. **O resto do GuidosToolbox.** O C++ está em `extern/terraflux-py/src/cpp/gtb` e a biblioteca `gtbcore` já é
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
     (`terraflux_ops::landscape::Product`; os do landschaft estão em `src/terraflux/_plotting/_palettes.py`:
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
     e os arquivos de ligação, o pós-processamento), os mesmos rank maps do z5 nos 65 projetos do landschaft.
   - **Feito** (Unreleased): as saídas nos arquivos do z5 (`output` em `prioritize.zonation` e
     `prioritize.zonation_project`: `rankmap.tif`, `feature_curves.csv`, `summary_curves.csv`,
     `feature_density_curves.csv`, `group_curves.csv`, `features_info.csv`), as mesmas linhas do z5 em 64 casos
     (as somas do Eigen em float). Os arquivos do pós-processamento (`post_processing/`: `area_stats`,
     `coverage_matrix`, `highest_occurrences`, comparação, histogramas, máscaras e mapas de flexibilidade) e a pasta
     `impacts/` saem pelo `output` do `prioritize.zonation_project`, iguais aos arquivos do próprio z5 nos 4
     projetos de pós-processamento e nos 5 de impactos. Falta: escrever o pós-processamento também por
     `prioritize.zonation` (sem projeto), e conferir com o z5 as unidades de área/impacto dos cenários e as listas
     de features com nomes (nenhum caso de referência as usa).

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
- O Viewer como central de mapas (camadas, comparação, os Plots no lugar do nó `view.plot`): o plano em
  "O Viewer como central de mapas, e os Plots no lugar do nó `view.plot`".

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
   **Feito** (Unreleased) o mesmo nos corpos dos laços e do switch: o ramo do switch guarda os nós como um grupo
   (`Engine::switch` dá um `Scope` ao corpo; um nó congelado dentro de um ramo é segurado), e os nós do corpo de um
   `control.repeat`/`control.each` ficam guardados por iteração (`Cache.iters`, chave: lugar e iteração; a impressão
   inclui o conteúdo dos valores que o nó lê de fora, `@v`, `@step`, `@item`, `@index`, `@name` e os `node.port` de
   fora, `content::hash_value`, 0,26 ms para um mapa de 1000x1000 em release): um laço rodado de novo com mais um
   passo, ou um `each` com uma camada mudada, roda só os nós do que mudou; `raster.write`, geradores e `partial`
   rodam sempre. Limite em RAM: 100000 entradas e 1 GiB das saídas, as menos usadas saem primeiro
   (`Cache::limit_iters`). Um laço tirado do cache conta os passos como contou (`LoopSteps` no `Kept`). Falta:
   congelar o ramo de um switch inteiro (só os nós dentro dele); um laço congelado lido do disco não tem os passos
   exatos (os reconstrói de uma série ou de uma pilha).
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
   relatório (HTML e PDF); ~~comparar mapas de sistemas de coordenadas diferentes sem `raster.match`~~ (feito:
   cortina, lado a lado e diferença levam o outro mapa à grade do mostrado; o teste
   `maps_of_two_coordinate_systems_are_compared_every_way`; um mapa sem CRS é recusado por `reproject::refusal`, que
   manda dar-lhe um com `raster.reproject` e só sugere `raster.match` para o resultado célula a célula).
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
   Falta: PDF (o SVG abre no Inkscape e sai em PDF), um mapa de vetores por cima, o encarte de localização (esses
   três e a fonte e a legenda à mão estão no item 6 de baixo).
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
   válidas, os renderizadores, as classes, o CRS do projeto). O `.qgz` (zip) já sai (ou `.qgs`). Também **feito**: "Layers: one GeoPackage" na
   janela (`export_as`): os vetores como tabelas e os mapas como rasters do GeoPackage (inteiros em tiles de 16 bits,
   o resto em Float32 com `TILE_FORMAT=TIFF`, `RASTER_TABLE`, `APPEND_SUBDATASET`), o projeto apontando para dentro
   (`GPKG:./arq.gpkg:tabela`); desligado, com o motivo, sem o driver GPKG. Falta: as cores e espessuras dos
   vetores do Viewer ("Vectors over the map"), os mapas por cima com sua opacidade, e a legenda contínua com rótulos
   escolhidos.
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
   **Feito** depois (1.53.0): cerca de 85 % dos operadores, por família (`crates/app/src/python/`). **Feito**
   (Unreleased, 2026-11-16): o `ml.*` como o scikit-learn, o `sdm.maxnet`, as randomizações e propriedades do
   Biodiverse, o `metrics.sample` (`python/models.rs`); as ferramentas gerais de raster (estatísticas, focal,
   majority, sieve, label, expand, shrink, classify, terrain do gdaldem, legend), as pilhas, os checks, o
   `value.calc`, o `rng.numpy`, os agentes, o `batch.run` e o `control.submodel` como funções auxiliares escritas no
   script (`python_tools.py`, `python/tools.rs`), conferidas contra os operadores (64 casos de mapas e 39 de checks,
   valores e agentes, nenhuma diferença; o terreno contra o gdaldem 3.12); o `data.stac` como `tf.download.stac`.
   Ficam como comentário, com o motivo: o que é do GDAL (`raster.fill`, `raster.mosaic`, `raster.contour`,
   `distance.proximity`), o Jenks do `raster.classify`, o `rng.r`, o `crafty.allocate`, `vector.density`,
   `vector.exact_sum`, `woe.weights`, `change.suggest`, `metrics.correlation`, os passos que a biblioteca só roda
   como modelo inteiro (MOLUSCE, CLUMondo, FUTURES, PLUS, Dinamica, lulcc, CARS), o `plugin.wasm` e o `view.plot`.
9. **Paletas para daltônicos.** viridis e cividis à mão, e uma prévia do mapa como um daltônico o vê (deuteranopia,
   protanopia, tritanopia); as revistas cobram cada vez mais.
   **Feito** (Unreleased): as rampas cividis e PuOr (laranja a roxo, centrada no 0) nas cores do Viewer; View >
   Colour vision (`palette::Vision`, as matrizes de Machado et al. 2009 em RGB linear) em todo mapa, legenda e
   imagem, dito na barra do alto até voltar ao normal. As classes já começavam pelas cores de Okabe e Ito.
   **Feito também** (Unreleased): os gráficos na visão escolhida (`palette::seen` em cada cor que entra num gráfico:
   a tabela de gráficos, a área das classes por passo, a série, a calibração, os perfis, os pesos de evidência, a
   sensibilidade, o orçamento de ganhos e perdas); e o aviso na legenda das classes quando duas cores ficam parecidas
   (CIEDE2000 abaixo de 10, na visão normal e na escolhida: "floresta e pasto parecem iguais para deuteranopia"), com
   um botão para as cores seguras de Okabe e Ito (`Palette::Safe`, também no menu Colours). Acima de oito classes
   nenhuma paleta separa todas para todos os leitores: as extras são as mesmas clareadas.
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
   **Feito** (Unreleased): editar os vértices de uma camada já lida (`vector.read` de um arquivo, inteiro, sem
   `extent`, no sistema de coordenadas do mapa): "Edit the features" no menu da linha da camada (`vector_edit.rs`:
   `Edit`, que guarda as feições em coordenadas do mapa, f64, com buraco e multiparte intactos; arrastar, ponto no
   meio da aresta, botão direito, "Add a feature", × na tabela, atributos tipados, Undo/Redo e Ctrl+Z). Grava um
   GeoPackage novo (`<nome>_edited.gpkg` ao lado, ou onde o "Browse..." disser), nunca por cima do original, lido
   por um nó `vector.read` novo; os campos inteiros continuam inteiros (`write_vector_as`, `integer_fields`).
   Falta: "Save over the original" (GeoPackage/Shapefile, em modo update, com cópia `.backup`); editar camadas em
   outro sistema de coordenadas que o do mapa; Z e M se perdem.
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
   demais). A fonte e a legenda e a barra à mão: **Feito** (Unreleased): `Layout::look` (`Look`: `Face` Source Sans 3,
   Inter ou a Ubuntu Light do egui; o tamanho-base do texto, 7,5 pt como antes, que multiplica o título, as legendas,
   as coordenadas, a seta e a barra; `legend_at` e `scale_at`, um `Place`: `Auto` como antes, um canto, ou `At(x, y)`,
   frações 0 a 1 do espaço livre). O SVG leva o `font-family` da fonte; o PDF embute a fonte inteira (Type 0,
   Identity-H, com `ToUnicode`: o texto busca e copia) no lugar da Helvetica; o PNG desenha os contornos dela. A
   legenda à mão fica sobre a figura (sobre o mapa de cada painel quando as legendas são por mapa), em fundo claro;
   a barra, dentro do mapa. Na prévia arrasta-se cada uma (um quadro mostra onde cai; perto de um canto, gruda); a
   janela tem Font, Text size e, para cada uma, Default, os cantos ou Custom (x e y em %). `Look` fica nas
   configurações (`figure_look`) e vale para a próxima figura. Falta: subconjunto da fonte no PDF (hoje ~200 KB a
   mais), uma serifa (nenhuma embutida), a legenda e a barra por painel com lugares diferentes. O encarte num mapa sem
   CRS conhecido **não** foi feito: sem CRS não há lugar no mundo a marcar (as células podem ser metros, graus ou só
   números de célula); só valeria com o usuário dando o CRS ou um ponto.
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
   do cache, a não ser congelado). **Feito** (Unreleased) também os laços dentro de grupos e de outros laços:
   `Session::loop_step` e `bodies` são pelo lugar do laço (`g/loop`, `outer/inner`), `loops_of` acha os laços no
   caminho do que o viewer mostra e cada um ganha o seu seletor (com o nome do laço quando são vários); vale o passo
   do laço mais de dentro com um escolhido (os de dentro dele no último), e o `Sending` do `runner` só manda o corpo
   do laço de dentro no passo escolhido do de fora (conta os corpos que o de fora já deu). O estado de um nó de dentro
   de um contêiner é pelo lugar inteiro (`Session::inner_status`, `status_at`; a tela do contêiner o mostra).
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

## O Viewer como central de mapas, e os Plots no lugar do nó `view.plot`

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
  opacidade, comparação, gráfico de uma tabela) é o que uma lista de camadas guarda. Ele vira um **Plot** salvo
  (`[[plots]]` no arquivo do modelo, fora dos nós), feito no próprio Viewer (*Save as plot*), e é ele que o
  Document, o relatório, o QGIS, o script Python, a comparação de cenários e as figuras leem.
- **Os modelos antigos continuam abrindo e rodando:** os `view.plot` viram Plots ao abrir (migração), e o
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
   **Feito** (Unreleased): `builder::show_in_viewer` (o duplo clique): seleciona o nó, `Session::ask_viewer` põe o
   Viewer na frente, e um nó do nível de cima sem resultado roda até ele por `Session::run_to_show`, quieto como a
   rodada depois de uma edição (só quando pronto; a falha dita por uma rodada pedida). O `poll` não abre mais abas
   dos plots; um template começa com o seu último plot no Viewer (`start_model`). As abas `Map` só se abrem ainda
   por `Session::open_map` (o código, até a fase 3). Testes: `canvas.rs` (o duplo clique), `ui.rs` (o plot, o
   template e o gráfico de uma tabela no Viewer, sem abas).
   A descrição do `view.plot`, as notas dos templates e os atalhos (F1) já dizem "no Viewer". Falta: os tutoriais do
   site ainda dizem "a tab of its own" (uns 40 trechos), reescritos na fase 4 junto com os Plots. Os do duplo clique já
   foram corrigidos na fase 2 (`tour.md`, `plots-and-tables.md`, `mspa.md`, `terrain.md`,
   `wetness-and-connectivity.md`, `depressions-and-flow.md`, `connectivity-as-a-graph.md`); ficam os dos `view.plot`.
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
   **Feito** (Unreleased), uma primeira versão: `builder::Layer` (nó, porta, visível, opacidade, estilo dos vetores)
   e `Session::preview_layers` no lugar de `preview_overlays` e `preview_rasters`; a seção **Layers** do painel do
   Viewer no lugar de "Maps over it" e "Vectors over the map": o mapa mostrado em itálico com 📌 quando não é uma
   camada, as camadas de cima para baixo (visível, o nome que, clicado, faz dela o mapa mostrado, com a sua legenda
   e ajustes; ⏶ ⏷, tirar; a opacidade de um mapa, o estilo de vetores) e *Add a layer* com os outros mapas e vetores
   do run. O duplo clique num nó fixa o seu mapa. Desenho: as camadas antes do mapa mostrado vão por baixo dele
   (`RasterLayer::below`), as de depois por cima; o mapa mostrado que não é camada vai por cima de todas; os vetores
   sempre por cima. O cursor diz o valor de cada mapa pelo nome (`RasterLayer::name`, `MapView::name`). As camadas
   seguem um nó renomeado e saem com um removido; o projeto do QGIS leva as camadas visíveis. Teste: `ui.rs`
   (fixar, clicar no nome, ⏷, tirar), `canvas.rs` (o duplo clique fixa). O painel próprio à esquerda feito (o botão *Layers* no cabeçalho o
   esconde; teste no `maps_stack_and_cells_show_their_series`). **A medida do Viewer com 1 e 5 camadas feita** (Unreleased): `ui.rs`, `viewer_timings` (`cargo test --release -p
   terraflux-app --test ui viewer_timings -- --ignored --nocapture`), análogo ao `canvas::timings` mas do Viewer: 4
   mapas `nlm.random` de 4000 x 4000 células e 300 polígonos; com 1 camada o mapa, com 5 os 4 mapas (3 a meia
   opacidade) e os polígonos. Um quadro é um `Harness::step` (a interface, os ladrilhos; não a pintura da GPU). Rodado
   uma vez em release com `RAYON_NUM_THREADS=2`, enquanto outro agente compilava (os números são indicativos), ms
   (mediana / p95): 

   | camadas | 1º desenho | parado | arrastando | zoom (roda) |
   |---|---|---|---|---|
   | 1 | 26 a 97 | 5,2 / 5,7 | 5,3 / 5,7 | 5,4 / 10,3 |
   | 5 | 25 a 71 | 5,5 / 6,4 | 5,4 / 6,3 | 5,3 / 16,0 |

   (o 1º desenho, do pedido até nada ocupado, variou entre as duas rodadas, 97 e 26 ms para 1 camada, 25 e 71 para 5:
   depende de os ladrilhos do mapa já estarem feitos; o `render` com a leitura de volta da wgpu, 1 ms.) Nenhum caso
   passa de 30 ms por quadro: os ~5 ms parados são o chão do quadro da aplicação inteira (os painéis), e o p95 do
   zoom com 5 camadas (16 ms) é o refazer dos ladrilhos dos vetores e dos mapas. Falta: medir em debug não vale; e
   com a GPU de verdade (aqui a wgpu do teste). Os testes por pixels:
   `the_layers_change_the_picture` (por cima, meia opacidade, escondida, por baixo).
3. **Comparar e dividir dentro do Viewer; o fim das abas `Map`.** (média)
   - Cortina entre a camada de cima e a de baixo (ou duas escolhidas); a diferença (`difference.rs`) como camada
     derivada.
   - Dividir em 1, 2 ou 4 vistas ligadas (zoom e lugar juntos, o cursor marcado nas outras), cada uma com a sua
     lista de camadas (ou a mesma com outra banda ou passo); tocar a série junto.
   - Saem `Pane::Map`, `map_tabs`, `beside_viewer`, `opened`/`quiet`/`raised` (`LAYOUT_VERSION` 12: um layout
     guardado com abas `Map` abre sem elas).
   - A figura (`figure.rs`) e o GIF/MP4 (`animation.rs`) partem das camadas e da divisão do Viewer.
   **Feito** (Unreleased), em parte: saíram `Pane::Map`, `map_tabs`, `Session::open_map`, `place_map_tabs`,
   `beside_viewer` e `opened`/`quiet`/`raised`. O Viewer é o único lugar de um mapa. O `LAYOUT_VERSION` ficou 11:
   as abas `Map` nunca eram guardadas (`comes_when_due`), então nenhum layout salvo as tem e nenhum precisa ser
   perdido. A comparação já era do Viewer (cortina, lado a lado, diferença com qualquer outro mapa do run ou outra
   camada de uma pilha). Agora as camadas fixas vêm primeiro na lista de *Compare*, a de cima primeiro. Os testes
   que abriam abas (`ui.rs`: o traço de uma célula, as legendas dos plots, o clique direito; `readme_images.rs`)
   usam o Viewer. Depois da 1.34.0: a figura e o GIF/MP4 levam as camadas de mapas (`animation::composed`,
   `Panel::rasters`, `Frame::rasters`; teste `the_layers_are_drawn_with_the_map`), como já levavam os vetores.
   Depois da 1.35.0: até 4 vistas lado a lado (`Compare::more`, `Beside`, *Another view* no Compare; 3 numa
   linha, 4 em duas), todas seguindo a que se moveu (`follow_all`), o cursor marcado nas outras, numa colorização
   só; o Plot as guarda (`PlotCompare::also`, conferido no `check`, seguindo nós renomeados e agrupados, no
   `set_plot`); a figura "the views side by side"; o traço de uma célula de qualquer vista. Testes:
   `views_side_by_side_follow_each_other` (4 vistas), `maps_compare_by_a_curtain_and_side_by_side` (duas linhas,
   a que se moveu leva as outras; o Plot com `also`), `plots_are_set_and_removed`.
   A diferença como camada (`Layer::minus`, `PlotLayer::minus`): *Keep as a layer* no Compare, feita numa thread
   (`layer_difference`, guardada em `Session::layer_differences` pelos mapas de que é), com a sua cor, legenda e
   resumo nos Layers, opacidade, *zoom*; num Plot, `minus` (conferido no `check`, no `set_plot`, seguindo nós
   renomeados e agrupados, saindo com um removido); o Document e o QGIS a deixam de fora. Testes:
   `a_difference_is_a_layer`, `maps_compare_by_a_curtain_and_side_by_side` (o botão, o desenho, o Plot).
   Tocar a série junto (`step_together`): no *Play*, as vistas de outros passos da mesma série andam com ela, cada
   uma à sua distância (teste `views_play_together`).
   Cada vista com as suas camadas feito (Unreleased, por um sub-agente): lado a lado, cada vista desenha as camadas
   do Viewer (o que era) ou uma lista sua. `Compare::layers` (a vista do mapa comparado) e `Beside::layers` (as
   vistas a mais) são `Option<Vec<Layer>>`, `None` as do Viewer; no Compare, uma dobra por vista (`view_layers`, em
   `beside_controls`): "same as the map shown" / "its own" (começa como cópia das do Viewer), mostrar/esconder,
   opacidade, tirar e *Add a layer* com os mapas e feições da corrida. As camadas de diferença (`Layer::minus`)
   também valem na lista de uma vista (o mesmo `layer_difference`; `Session::layer_differences` só larga as que
   nenhuma lista usa). `draw_layers` (tirado do `output`) monta os rasters e as feições de uma lista, e o
   `side_by_side` recebe os das vistas com lista própria; o contorno da seleção (`MapView::selected`) só vai às
   vistas que desenham as camadas do Viewer. No Plot, `PlotCompare::layers: Vec<Vec<PlotLayer>>` (serde default,
   omitido quando vazio; uma lista por vista, a de `with` primeiro, depois as de `also`; lista vazia: as camadas do
   Plot), levado por `plot_of`/`open_plot`, conferido no `check` (`plots.{id}.compare.layers[v][i]`), seguindo nós
   renomeados e removidos (`PlotCompare::each_ref`/`retain_refs`, `preview_follow`, o `plots_follow` do Document) e
   aceito pelo `set_plot` (`compare.layers`, sem `palette`: um mapa tem uma só em todas as vistas). Testes:
   `a_view_has_layers_of_its_own`, `the_layers_of_the_views_are_kept_and_follow_the_nodes`,
   `a_view_side_by_side_has_layers_of_its_own` (`MapView::layers_drawn`), `plots_read_what_the_model_has` e
   `plots_are_set_and_removed`. Falta: uma lista própria sem camadas não é guardada no Plot (volta às camadas do
   Plot); o estilo das feições e o histograma não aparecem na lista compacta de uma vista.
   A diferença de duas camadas de uma pilha feita (Unreleased, por um sub-agente): o *Keep as a layer* vale também
   para dois passos da mesma pilha ou série (`CompareWith::Layer`) e, nas saídas, usa a banda mostrada; a
   `Layer` ganha `minus_band` (a banda do mapa subtraído; `band` é a do outro), a `DifferenceKey` leva as duas
   bandas (duas diferenças da mesma pilha com outras bandas ficam em cache à parte), `layer_difference` pega cada
   lado com `layer_source`; o nome sai como "x.map step 5 minus step 1" (`Layer::name_by`, com os nomes das
   bandas da pilha ou "band 5"); no Plot, `PlotLayer::minus_band` (serde default, omitido quando vazio), levado
   por `plot_of`/`open_plot` e aceito pelo `set_plot` (só com `minus`). O `step_together` só mexe nas vistas
   do Compare, nunca nas camadas de diferença. Testes: `two_bands_of_a_stack_are_a_difference`,
   `the_difference_of_two_bands_of_a_stack_is_a_layer`, `plots_are_set_and_removed`, `plots_read_what_the_model_has`.
   Feito depois (Unreleased): a banda da outra saída comparada. `Compare.band` e `Beside.band` (um `band_choice`
   no Compare e em cada vista a mais, só quando a saída tem vários mapas; trocar a saída volta à primeira) vão
   a `compared_map`/`output_map`; o *Keep as a layer* de uma saída usa essa banda como a `band` da camada; o
   `PlotCompare` ganhou `band: Option<usize>` e `also_bands: Vec<usize>` (0 = a primeira, TOML não guarda `None` em
   lista; omitidos quando vazios; `retain_refs` os mantém junto de `also`), levados por `plot_of`/`open_plot` e
   aceitos pelo `set_plot` (`band`, `also_bands`). Testes: `the_layers_of_the_views_are_kept_and_follow_the_nodes`,
   `plots_are_set_and_removed`, `comparing_with_a_band_of_another_stack`. Falta: os nomes das bandas no
   `check_model` (uma banda além das que a saída tem vale a última, sem aviso).
4. **Os Plots salvos no lugar do nó `view.plot`.** (a maior)
   - Formato: `[[plots]]` no modelo (`terraflux_graph::Model::plots`, serde, omitido quando vazio): `id`, `title`,
     `layers = [{ from = "node.port", opacity, palette, band, style }]`, `compare = { with, how }`,
     `chart = { kind, x, y, by }` para tabelas, extensão opcional, `note`/`why` (o registro). O `check` confere as
     referências (`plots.<id>.layers[i]`); renomear e remover nós atualizam os Plots (`editor.rs`).
   - Migração ao ler (`terraflux_graph`): cada nó `view.plot` vira um Plot (`value`/`features`/`table` → camadas, a
     cadeia de `under` → as de baixo, como `plot_stack`; `opacity`, `compared`/`compare`, `chart`/`x`/`y`/`by`,
     `title`); o nó sai, e o que lia `plot.shown` passa a ler o que o plot mostrava. Salvo, o arquivo já tem
     `[[plots]]`. O operador `view.plot` fica obsoleto (fora da biblioteca, `describe_operator` diz que o plot agora
     se faz no Viewer) até a 2.0.
   - Quem lê os nós plot passa a ler os Plots: `document.rs` (as figuras de cada seção), `params_view::comparable`,
     `legend_view` (sem o salto pelo plot), `batch_view`, `report.rs`, `qgis.rs` (um grupo de camadas por Plot, com
     os estilos), `python.rs`, `figure.rs`, `methods`, `advice.rs` (o conselho que sugeria um nó plot sugere um Plot),
     `op_notes`, `phrases` (a frase do Plot no Document).
   - No app: *Save as plot* no painel de camadas, a lista dos Plots no Viewer e no Document; abrir um Plot carrega
     as suas camadas; *Update plot* grava as mudanças.
   - Templates: os 101 `view.plot` de `templates.rs` viram Plots (pela própria migração, conferida à mão nos
     títulos). MCP: `set_plot` e `remove_plot` (`tools.rs`, `terraflux mcp`), o guia `results`, o assistente; a
     tradução de scripts Python cria Plots. Site: 134 menções em 47 tutoriais; `plots-and-tables.md` reescrito como
     "Plots and figures".
   - Testes: cada template migrado dá as mesmas camadas que o `plot_stack` dava; um modelo antigo com `view.plot`
     roda no CLI; o Document, o relatório e o QGIS dos templates iguais antes e depois.
   **4a feito** (Unreleased): o formato e a migração, sem ligar ainda.
   - `terraflux_graph::plots`: `Plot` (`id`, `title`, `layers`, `compare`, `chart`, `note`, `why`),
     `PlotLayer` (`from`, `opacity`, `palette`, `band`, `style`, `hidden`), `PlotCompare`, `PlotChart`, e
     `Model::plots` (`[[plots]]`, omitido quando vazio).
   - `Model::plots_from_nodes`: os `view.plot` do nível de cima viram Plots (a cadeia de `under`, o mapa com a
     opacidade, os vetores, a tabela com o gráfico, `compared`/`compare`, título, nota e porquê). O que lia
     `plot.shown` passa a ler o que o plot mostrava; os parâmetros e cenários de um plot saem. Os de dentro de
     grupos ficam nós.
   - O `check` confere os ids e o que as camadas e a comparação leem.
   - Testes: no grafo (a migração, o arquivo, o plot vazio e o de dentro de um grupo, o `check`); no app, os
     templates (as mesmas camadas que o `plot_stack` dava, mais a tabela) e os 47 modelos do site.

   **4b feito** (Unreleased).
   - A migração ligada no `Model::upgrade` (todo modelo lido) e no `Session::edited` (um `view.plot` posto
     pelo assistente ou por um agente vira Plot).
   - O Viewer: o menu **Plots** no cabeçalho (`builder::plots_menu`): abrir (`builder::open_plot`: as camadas, o
     mapa mostrado e a banda, a comparação; uma tabela no gráfico do Plot), *Save as a new plot*
     (`plot_<nó>`), *Update the plot*, o título, *Remove the plot*. Um template começa com o seu último Plot
     aberto. O estilo dos vetores guardado como JSON (`Style::to_json`/`from_json`).
   - As referências dos Plots seguem um nó renomeado ou agrupado e saem com um removido (`editor::for_each_ref`,
     `remove_node`).
   - Os leitores (feitos por um sub-agente num worktree à parte): o Document (`document::plotted`: a camada de
     cima que não é vetor; as figuras na ordem dos Plots), o relatório, a comparação de cenários (o mapa de cada
     Plot), o *Many runs* e o script Python. O QGIS não lia os plots.
   - Testes: `ui.rs` (salvar um Plot pelo menu, a comparação de um Plot, o template, as camadas de um Plot, a
     tabela no gráfico, as legendas dos templates), os do Document e do relatório.
   Depois da 1.34.0: a banda e a paleta por camada (`Layer.band`, o seletor "map:" nos Layers; `plot_of` grava a
   paleta escolhida à mão pelo nome do Viewer, `open_plot` a devolve, `palette_of` cai na do Plot para o Document;
   `builder::palette_name`/`palette_named`; teste `a_layers_band_and_palette_are_kept_in_its_plot`).
   *Zoom to layer* (o "zoom" de cada camada: `MapView::zoom_to`, `builder::layer_cells`; teste no
   `maps_stack_and_cells_show_their_series`).
   O identificar diz a feição de cada camada de vetores sob o cursor, pelo nome da camada (`VectorTiles::pick_all`,
   `map_view::feature_text`, `Overlay::name`; teste `the_features_of_every_layer_are_told`).
   Arrastar uma camada pelo nome para a linha de outra (`LayerAt`, o `dnd` do egui; teste no
   `maps_stack_and_cells_show_their_series`).
   A lista dos Plots no Document (a seção "Plots": título, camadas, comparação, a figura; "open" e "open in the
   Viewer" nas figuras abrem o Plot; `Document::figure_of`).
   Falta: a coluna inteira por Plot na comparação de cenários (hoje o mapa de cada Plot; a confirmar, ver "Em
   aberto").

   **4c feito** (Unreleased).
   - O MCP e o assistente: `set_plot` (camadas como `node.port` ou `{ from, opacity, band, hidden }`, `compare`,
     `chart`, título, nota, porquê; referências conferidas) e `remove_plot`. O guia `results` fala dos Plots.
   - O `view.plot` aposentado: fora da biblioteca (`builder::Library`), da busca e do `list_operators`; o
     `add_node` recusa com a dica do `set_plot`; o `describe_operator` e a descrição do operador dizem por quê.
     Fica registrado até a 2.0 (os plots dentro de grupos, os modelos antigos).
   - Os tutoriais do site, reescritos por um sub-agente num worktree à parte (40e9a8a, 52 arquivos): os blocos
     `tfgraph` com `[[plots]]` no lugar dos nós (o `tfgraph.py` só desenha os nós; o TOML mostra os Plots), a
     página `plots-and-tables.md` como "Plots and figures"; o site gerado sem avisos.
   Depois da 1.34.0: *Save as a new plot* e *Update the plot* gravam o gráfico escolhido no Viewer para uma tabela
   (`builder::chart_names`, pelos nomes das colunas; teste em `a_table_is_plotted_and_written`); os 23 trechos
   "under the canvas" do site corrigidos (4b4e6c8).
   As figuras do tour refeitas (`empty.png`, `window.png` com os números, `model-pane.png`) pelo teste
   `readme_images::tour_images` (os números postos pelos lugares dos botões, `--ignored`), e as legendas e os
   itens 6 e 8 do tour postos em dia. O diagrama de `fitting-models.md` já era `fragment: true` (o site sai sem
   avisos): nada a fazer.
5. **Depois** (a decidir, cada um à parte): reprojetar outro CRS por tile, ao desenhar; medir distância e área;
   soltar um arquivo no Viewer (o `raster.read` criado e a camada fixada); a tabela de atributos de uma camada de
   vetores; rótulos; o histograma de cada camada; exportar o Plot como PNG ou GeoTIFF composto.
   **Medir feito** (Unreleased, por um sub-agente num worktree à parte): `measure.rs`, o botão *Measure* do Viewer
   (ou M sobre o mapa) liga uma linha clicada ponto a ponto, com cada trecho e o total e, de três pontos, a área e
   o perímetro, em m, km, m², ha e km². Em grades projetadas as contas são planas (com a unidade linear da grade);
   em graus, geodésicas na elipsoide WGS84 (a distância de Vincenty, a área na esfera autálica); sem CRS, nas
   unidades da grade e em células. Clique direito ou Backspace tira o último ponto, o duplo clique encerra, Esc
   limpa, *Copy*; medir e desenhar feições são exclusivos. A caixa dos números no canto de baixo do mapa. Testes:
   os de `measure.rs` (1° no equador, o caso de Vincenty, um octante da Terra) e `measuring_on_the_map_by_clicks`
   (cliques de verdade). **Conferido com um arquivo em graus** (Unreleased): o `gdal_translate -a_srs EPSG:4326
   -a_ullr 122.30 12.60 122.62 12.33` do `sibuyan_landuse.tif` (128 x 108 células de 0,0025°) lido e medido no Viewer
   (`measuring_a_file_in_degrees_matches_the_references`; em `measure.rs`, `a_file_in_degrees_measures_as_the_references`
   com um GeoTIFF escrito pelo `terraflux_raster`). Referências de fora, `pyproj` 3.7.2 `Geod(ellps="WGS84")`
   (GeographicLib, geodésicas exatas) e o GDAL (`ogrinfo -dialect SQLite -sql "SELECT ST_Area(GeomFromText(wkt,
   4326), 1)"`): entre os cantos das células (10, 20) e (110, 90) 33 368,4751 m, e o Viewer 33 368,47 m (Vincenty,
   dentro de 1 cm); o retângulo (10, 20)-(110, 90), 526 161 498,514 m² (pyproj e GDAL dão o mesmo) e perímetro
   93 075,771 m, o Viewer 526 161 494,39 m² (diferença de 8e-9: a esfera autálica tem a área da elipsoide, as
   arestas de grande círculo mal mudam o polígono de 27 x 19 km) e 93 075,7707 m. Os limites pedidos eram 0,1% e
   0,5%. Nenhum erro achado. Um clique de verdade bate com a haversine numa esfera dentro de 0,5%. **A linha segue
   as vistas lado a lado** (Unreleased): `MapView::paint_measure` desenha a linha da vista medida nas outras, cada
   ponto levado pelo `reproject::Link` (também entre sistemas de coordenadas; teste
   `the_measure_line_follows_into_the_views_side_by_side`, UTM e WGS 84 lado a lado, por pixels). Falta: a caixa dos
   números só na vista medida; medir clicando numa vista ao lado mede ali, com a sua própria linha.
   **Soltar um arquivo no Viewer feito** (Unreleased, por um sub-agente): `builder::drop_on_viewer`. Os arquivos
   soltos sobre o Viewer (`TerrafluxApp::viewer_rect`, `over_viewer`) criam o `raster.read`/`vector.read`, fixados
   como camadas (os mapas na ordem soltos, os vetores por cima), o primeiro mostrado se o Viewer estava vazio, e o
   modelo roda até cada um (`Session::show_queue`, uma rodada por vez). O aviso cobre só o Viewer enquanto estão
   sobre ele. No Windows o winit não diz onde está o ponteiro durante um arraste do Explorer: a posição vem do
   sistema (`system_pointer`, `GetCursorPos`). Testes: `files_dropped_on_the_viewer_become_layers_of_it`,
   `over_the_viewer_is_told_by_where_it_was_drawn`. Falta: conferir arrastando de verdade do Explorer; um Viewer
   numa janela à parte.
   **A tabela de atributos feita** (Unreleased, por um sub-agente): `attribute_table.rs`. O botão *table* na linha
   de uma camada de vetores (e *Table* para vetores mostrados sozinhos) abre sob o mapa uma tabela com uma linha
   por feição (o número e os atributos; os textos ordenados pelo nome), ordenável e filtrável como as outras
   (`table_view::show_picking`), *Copy* (por tabs) e *Save as CSV*. A seleção ligada: a linha clicada destaca a
   feição no mapa (`MapView::selected`, o contorno na cor de destaque) e leva a vista até ela; a feição clicada no
   mapa (`MapView::feature_clicked`) seleciona a linha e rola até ela. Testes: os de `attribute_table.rs`,
   `the_attribute_table_of_a_vector_layer`. (Várias feições e as vistas lado a lado: abaixo.)
   **O histograma feito** (Unreleased, por um sub-agente): `histogram.rs`. Sob a legenda de um mapa de valores, 60
   barras entre os limites da colorização, na cor da rampa, com mínimo, máximo, média e as células com e sem valor
   (o cursor diz a faixa e a contagem de uma barra); feito numa thread e guardado pela fonte (`Histograms`, como
   os `Looks`), de uma amostra de até 4 milhões de células ("from a sample of N cells"). Nos Layers, um
   *histogram* recolhido em cada camada; a legenda de classes com uma barra por classe. Testes: os de
   `histogram.rs`, `the_histogram_of_a_map`. Falta: as barras em escala logarítmica quando a cor é; o nodata que
   não é NaN de um raster em disco conta como valor (como no `look_of`).
   **Os rótulos feitos** (Unreleased, por um sub-agente): `labels.rs`. *labels* no estilo de uma camada de
   vetores (o atributo e o tamanho, 9 a 20 pt): polígonos num ponto de dentro (o centroide quando dentro, senão o
   meio do trecho de dentro mais longo), linhas no meio, pontos ao lado, num halo fino. Só as feições à vista e
   grandes o bastante para o texto; um rótulo que cobriria outro sai (`Placed`, uma grade de colisão); no máximo
   2 000 por quadro. Os pontos dos rótulos feitos uma vez por camada, com o índice (`vector_tiles::Layer`). O
   `Style` guarda `labels` e `label_size` (o JSON antigo continua lido); a figura também os desenha. Testes: os de
   `labels.rs`, `a_layer_knows_where_its_features_are_labelled`, `the_labels_of_a_vector_layer`, o SVG da figura.
   Falta: o projeto do QGIS ainda tem os rótulos à parte (`VectorOptions`), não os do estilo; a prioridade pelo
   tamanho da feição.
   **Exportar a vista feito** (Unreleased, por um sub-agente): `export_view.rs`. *Export the view...* ao lado de
   *Figure...* grava o mapa mostrado com as camadas, os vetores e os rótulos como PNG (transparente sem valor) ou
   GeoTIFF RGBA com a grade e o CRS do mapa (`tiff::write_rgba`), um pixel por célula ou ×2, ×4, no máximo
   8 000 px no lado maior, numa thread. O desenho é o da figura (`figure::plain`, `pixels_on`). Testes: os de
   `export_view.rs` (os pixels, o GeoTIFF lido de volta), `the_view_exported_as_a_picture`. Falta: a vista
   comparada; o basemap.
   **Reprojetar ao desenhar feito** (Unreleased, por um sub-agente): `reproject.rs`. Uma camada de mapa (ou o mapa
   da cortina) noutro CRS que o mapa mostrado não é mais pulada: cada tile, feito nas células do próprio mapa como
   qualquer outro, vira uma malha de 16 × 16 quads cujos vértices são os cantos das células levados à grade
   mostrada (transformados uma vez e guardados, até 2 048 tiles), e a GPU estica a textura sobre eles; o valor sob
   o cursor vem pela transformação inversa (`crs_pair`, `key`). Teste:
   `a_layer_of_another_coordinate_system_is_drawn_reprojected`.
   Também feito: lado a lado (`Link::between` em `reproject.rs`: lugar quando o CRS é o mesmo, senão a
   transformação; `follow_all` leva o centro da câmera e tira a escala do deslocamento de uma célula, o cursor
   também; cada vista desenha o próprio mapa na própria grade, com as camadas dela reprojetadas), a figura e
   *Export the view* (`animation::composed`: cada pixel de saída levado à grade da camada por linha, vizinho mais
   próximo; os quadros da animação saem iguais) e o zoom à camada (`Reproj::box_on_view`, as bordas levadas).
   Testes: `views_of_two_coordinate_systems_follow_each_other`,
   `a_layer_of_another_coordinate_system_is_carried_to_the_map`,
   `views_of_maps_of_two_coordinate_systems_follow_each_other`.
   A diferença de mapas de dois CRSs também está feita (`difference::carried`: o outro mapa levado à grade do
   primeiro na CPU, por linha, com `Reproj::to_source_many`, vizinho mais próximo para classes e valores, NaN onde
   não há célula; a transformação é feita na própria thread da diferença; `follow` aceita `Link`). Testes:
   `a_map_of_another_coordinate_system_is_carried_to_this_ones_grid`,
   `the_difference_of_maps_of_two_coordinate_systems_is_made`. Falta: grades polares (nelas a longitude quase não
   significa nada, e o desembrulho abaixo não vale: uma camada polar numa vista geográfica cruza todas as
   longitudes, e os quads que pulam o antimeridiano somem; não se resolve com o desembrulho, pede cortar a malha
   no polo); ~~o seguimento de câmera~~ (feito: `calc_view::followed` usa `Link::between`, os cantos e o interior da
   vista levados à grade; o seguimento de câmera dos views lado a lado já usava `Link`).
   **Grades rotacionadas e o antimeridiano feitos** (Unreleased, por um sub-agente): `Reproj` (`reproject.rs`) passou
   a levar as células pela transformação afim inteira (`Affine`: cantos → mundo → transformação de CRS → células da
   grade mostrada pela inversa da afim dela), então uma grade rotacionada (do mesmo CRS ou de outro, ou a própria
   grade mostrada rotacionada) vai pelo mesmo caminho de malha (`Link::Warp`); `crs_pair` não recusa mais rotação, e
   com o mesmo CRS (ou nenhum dos dois dito) não há transformação do GDAL. Antimeridiano: quando o CRS mostrado é
   geográfico, os x levados são deslocados de 360° para o mais perto da longitude do meio da grade mostrada
   (`unwrap`), e na volta (`to_source_many`, o cursor, a diferença, a figura) a longitude da vista vai para o mais
   perto do meio da camada, então uma camada de 170 °E a 170 °W é buscada a ±360°; `mesh` descarta os quads cujos
   cantos ainda distam mais que meio mundo (`Reproj::half`, o corte a 180° do meio da vista). Mensagens de recusa sem
   "rotated". Testes: `a_rotated_grid_is_carried_to_an_axis_aligned_one_and_back`,
   `a_layer_across_the_antimeridian_is_carried_without_a_jump`, `a_layer_across_the_antimeridian_on_a_projected_view`,
   `a_rotated_layer_is_carried_to_the_map` (animation), `a_rotated_grid_is_carried_to_this_ones_cells`
   (difference), `a_rotated_map_layer_is_drawn` (UI).
   **Várias feições selecionadas feito** (Unreleased, por um sub-agente): `Pick` (`table_view.rs`) guarda as linhas
   (`rows`), a última clicada (`row`) e o começo da faixa (`anchor`): `Pick::click` (clique simples, Ctrl alterna,
   Shift pega a faixa na ordem mostrada), `clear`, `only`; `click_modifiers` lê as teclas do clique. A tabela mostra
   "N selected", *Zoom to selection* (a união dos `feature_box`, `union_box`), *Clear selection* (Esc também),
   *Copy selected* e *Save selected* (`subset`). No mapa, clique escolhe a feição e Ctrl+clique alterna
   (`feature_toggle`). `MapView.selected` virou `Option<(camada, Vec<feição>)>`: só as que estão à vista (pela caixa)
   são contornadas, no máximo 500 (`SELECTED_DRAWN`); `side_by_side` passa a seleção às vistas comparada e extras.
   Testes: `rows_toggled_and_ranged`, `selected_rows_as_a_table`, `selection_box_is_the_union`,
   `the_attribute_table_of_a_vector_layer` (Ctrl, Shift, Esc).
   Feito depois (Unreleased): `Pick::all(shown)` (*Select all shown*, também com nada selecionado, e Ctrl+A com o
   ponteiro sobre a tabela e nenhuma caixa de texto com foco; `table_view::shown_rows` dá as linhas que o filtro
   deixa); `Pick.plain` e `Pick::zooms` (só o clique simples leva o mapa à feição, um Ctrl+clique que deixa uma
   linha não); o clique numa feição de uma vista lado a lado que desenha as camadas do Viewer (não as que têm
   camadas próprias) seleciona a linha, como o do mapa principal. Testes: `all_the_rows_shown_selected`,
   `only_a_plain_click_zooms`, `the_attribute_table_of_a_vector_layer`. Falta: o Shift+clique no mapa
   (laço/retângulo para selecionar por área); a seleção guardada no Plot ou usada por `vector.filter`; Ctrl+A com
   o foco do teclado (não só o ponteiro) na tabela.
   **Exportar um Plot pelo id feito** (Unreleased, por um sub-agente): `plot_export.rs`. `terraflux plot <modelo> <id>
   -o f.png|f.tif [--scale 1|2|4] [--format] [--set] [--scenario]` e a ferramenta MCP `export_plot { model, id, file,
   scale }` rodam o modelo e gravam o Plot sem o app: `panel_of` monta o `figure::Panel` (o primeiro mapa visível é
   o mapa, opaco; os outros mapas por cima na opacidade de cada um; os vetores com o estilo e os rótulos; camadas
   escondidas, diferenças (`minus`) e sem valor puladas, com o motivo na resposta) e `write` usa `export_view::save`.
   As cores são as que o Viewer escolheria só pelo Plot: a `palette` da camada, senão `builder::default_palette`
   (antes dentro do Viewer: o produtor do nó, os nomes, o `landschaft_product`), as classes pela legenda do modelo
   (`legend_view::legend_in`, sem `Session`). Plot com `compare` grava só o mapa (dito na resposta). Testes: os de
   `plot_export.rs`, `a_plot_is_exported_by_its_id`, `a_plot_is_written_by_its_id`. Falta: as diferenças (`minus`),
   que no app rodam numa thread; a vista comparada; o gráfico de uma tabela; nós dentro de contêineres (só
   `node.port` do topo e `$entrada`); a ferramenta do assistente dentro do app.

### Em aberto

- **Nome: Plots** (decidido em 2026-11-03): o que já faz esse papel, e a migração natural (`view.plot` → `[[plots]]`).
- **Comparação de cenários** (`params_view`): hoje cada cenário mostra um mapa só (uma saída do modelo ou o valor de
  um plot) numa coluna. Com Plots de várias camadas, a proposta é cada coluna mostrar o Plot inteiro (as camadas e
  os estilos de quem o montou), e a diferença entre cenários continuar calculada só no mapa principal (a camada de
  baixo). A confirmar.
- Por quanto tempo o `view.plot` obsoleto fica (proposta: até a 2.0).
- As fases 2 e 3 trocam `Shown` e o `preview` da `Session`: o trabalho no Viewer feito em paralelo (o passo de um
  laço, 60b796c) entra nelas, e um trabalho ainda aberto ali precisa terminar antes.

## Paridade com o landschaft na priorização, progresso e parada

Registro de 2026-10-16. O `prioritize.optimize` passou a resolver como o landschaft (o motor de corte e o de
fecho antes do HiGHS, o Gurobi quando instalado), com barra de progresso e Stop no meio da execução. Ficou para
depois:

1. **O resto do roteamento do landschaft** (`prioritize/_milp_solve.py`), para a mesma velocidade em todos os
   problemas:
   - **Feito** (Unreleased): o motor de corte com zonas (`solve_zone_cut`, `opt/zones_cut.cpp`): mínimos conjuntos
     com zonas e penalidades de borda ou conectividade (`engines::zone_cut_model`, `engines::zone_cut`, o ramo em
     `milp::solve_from`; `ffi::opt_solve_zone_cut` com o `WatchScope`). Testes `zone_boundary_penalties_go_*`
     (`crates/ops/tests/zones.rs`): o plano, o bound e o status do landschaft 1.58.1, o motor sozinho (gap 0.3) e
     com o HiGHS depois (gap 0.15, ignorado, 7 s em release). Achado no caminho: no Windows o C++ todo era
     compilado sem otimização (o crate cmake troca as flags de Release do CMake pelas do cc, sem os `/O`): o
     `build.rs` do `terraflux-ls-sys` passa as do CMake (`/O2 /Ob2 /DNDEBUG`), 22,7 s caíram para 7,0 s (os do
     landschaft);
   - **Feito** (Unreleased): a busca por subconjuntos depois do motor largo (`_wide_then_subset`, `improve_subset`):
     `max_features` e as filogenias (`crates/ops/src/milp/subset.rs`, o ramo em `milp::solve_from`; testes
     `subset_*` e `the_search_goes_on_from_the_wide_engine_and_stops_with_the_run` em `crates/ops/tests/optimize.rs`:
     a mesma busca do landschaft a partir das mesmas seleções, o modelo compilado passado ao landschaft bit a bit).
     O caminho inteiro (o motor largo com limite de tempo, depois a busca) depende da velocidade: num caso 150x40 o
     landschaft foi de 19 a 20 alvos e o Terraflux ficou em 19, por partir de outra solução do motor largo;
   - **Feito** (Unreleased): comparar com o landschaft (1.58.1, HiGHS, roteamento padrão, 4 threads), nos mesmos dados,
     cada objetivo (os 10) em cada tipo de unidade (raster, tabela, polígonos, hexágonos H3 em duas resoluções, e
     zonas em raster e tabela com os objetivos que as aceitam), em dois conjuntos (os casos do landschaft, 12x15
     células, e outro de 40x50 para os tempos significarem algo): 114 casos. Ferramenta
     `tools/make_optimize_parity_reference.py` (escreve `tests/data/reference/optimize_parity.json`,
     `optimize_parity_inputs.npz` e `vectors/parity_*.gpkg`), teste `parity::optimize_is_landschafts` em
     `crates/ops/tests/parity.rs` (`#[ignore]`, release, `--test-threads 1`). Resultado: o modelo compilado, o estado
     e o objetivo iguais em todos; as unidades escolhidas iguais onde o ótimo é único; o tempo do Terraflux igual ao
     do landschaft (soma 139 s contra 141 s nos 114 casos; nenhum passa de 1,5x + 0,25 s). Os hexágonos do H3 vão
     ao Terraflux como o landschaft os fez: os casos ficam em x = 0 da zona UTM 23S (latitude -85), onde o PROJ 9.5.1
     (pyproj) e o 9.7.1 (GDAL do Terraflux) diferem em 1e-8 m, o bastante para mexer nas quantidades das células só
     tocadas por um hexágono (e o modelo, e o caminho do HiGHS, com elas). Os casos que o limite de tempo
     (8 s) corta, `min_penalties` com a penalidade de borda, só têm o tempo comparado;
   - **Feito** (Unreleased): o solver `anneal` (`add_anneal_solver`, `_milp_anneal.py`: o recozimento simulado do Marxan
     sobre o mínimo conjunto compilado, cada rodada reparada) como `solver = "anneal"` do `prioritize.optimize`, com
     `num_reps`, `num_iters`, `start_temp`, `cool_fac`, `num_temp`, `spf`, `itimp_type`, `seed` (padrão 1) e
     `time_limit`; recusa zonas, portfólios e os modelos que não são mínimo conjunto binário, com as mensagens do
     landschaft (`crates/ops/src/milp/anneal.rs`; o `opt_marxan` do `ls-sys` ganhou `fast_penalties`, `repair`,
     `time_schedule`, `spf_scale` e `time_limit`). Só o `prioritize.optimize` o aceita (o landschaft também só o
     aceita em `Problem.solve`; `engine_of` dos outros nós segue com `highs` e `gurobi`). Com semente, a mesma
     seleção e o mesmo objetivo do landschaft nos 6 casos `*_anneal_*` do `parity::optimize_is_landschafts`;
   - **Feito** (Unreleased): os corredores (`add_corridor_constraints`, `_apply_corridors`): `Spatial::Corridors`
     em `crates/ops/src/milp.rs`, o modelo de fluxo comparado bit a bit com o do landschaft (5 modelos,
     `corridors_compile_is_landschafts`, `tools/make_corridor_reference.py`) e o roteamento de
     `m.connected or m.corridors` (`ffi::opt_solve_corridors`: cortes de nó na raiz, depois o MIP do HiGHS com o
     separador nos nós, como `solve_connected`); os 2 casos `*_corridors_*` do `parity::` com a mesma seleção e o
     mesmo tempo. Não vão (como no landschaft) com zonas; a resistência como `Transition` do `ls.cost` não existe
     (só valores por célula ou coluna).
2. **O Gurobi além do `prioritize.optimize`:** **feito** (Unreleased) nas re-soluções do `prioritize.evaluate`
   (custo de reposição e rank: as configurações `solver` e `threads`, uma thread do Gurobi em cada re-solução em
   paralelo, teste `importance_is_prioritizrs`) e no `prioritize.calibrate` (candidatas em paralelo, uma thread do
   Gurobi em cada; teste `calibrate_on_gurobi_is_highs`) e no `restoration.plan` (os programas lineares
   com as threads do nó; teste `plan_on_gurobi_is_highs`) e no `prioritize.actions` e `actions_raster` (o
   modelo equivalente na curva 1, o de prioriactions com curvas nas outras; teste
   `optimum_on_gurobi_is_prioriactions`). Item feito por inteiro.
3. **Parar no meio e dizer quanto falta em outros operadores longos:** **feito** (Unreleased) nos laços em Rust: o
   `sdm.fit` dentro de cada ajuste (os passos de Newton do GLM, os lotes de árvores da floresta, as iterações do
   boosting), o `sdm.importance`, o `sdm.folds` e o `ml.fit` (`stop: &mut dyn FnMut() -> bool` nos ajustes; teste
   `sdm_fit_reports_and_stops` em `crates/ops/tests/stopping.rs`). O `prioritize.evaluate`, as métricas por blocos,
   o MSPA por tiles e o `circuit.one_to_all` já paravam. Falta o que é uma chamada só ao C++ do landschaft (o MSPA,
   os circuitos, o maxnet): registrado no IDEAS.md do landschaft ("Parar no meio fora da priorização"). Antes:
   hoje só os solvers param; um nó que não
   para (o `prioritize.evaluate`, o MSPA, os circuitos, o ajuste dos SDM, as métricas por blocos) é deixado
   terminar em segundo plano um segundo depois do Stop. Basta o operador chamar `ctx.stopped()` nos seus laços e
   `ctx.progress(...)`; no C++, perguntar `opt::stop_asked()` (ou um equivalente fora do `opt`).
4. **Feito.** **O submódulo do landschaft:** já está no `main` do landschaft (1.58.1, `295d123`, com o `WatchScope`).
5. **Testes instáveis vistos:** `a_computer_lends_itself_to_another_by_its_code` (`crates/app/tests/ui.rs`) falhava às
   vezes na suíte inteira e passava sozinho: **corrigido** (Unreleased; as configurações são globais no programa `ui`:
   `settings_themes_and_favourites` as zerava inteiras, o `pool_code` não era atômico e a janela Settings e o
   empréstimo regravavam tudo a cada quadro); `the_terrain_is_landschafts` (`crates/ops/tests/surface.rs`) difere em
   1e-16 no VRM só no build de debug (passa em release).
   Vistos em 2026-11-14 na suíte inteira (`cargo test --workspace`, 4 threads) e **corrigidos** (Unreleased):
   `a_model_runs_and_every_tab_draws` (`crates/app/tests/ui.rs`): abrir o modelo pede uma corrida inteira
   (`run_when_ready`) e o `can_run()` logo depois dava falso se ela já tivesse começado; o teste a desliga (ele
   corre pelo botão Run), e o `model()` escreve o arquivo uma vez só.
   `settings_themes_and_favourites` (`crates/app/tests/ui.rs`): o tema é global do programa e o quadro de cada
   app o escolhia com uma cópia das configurações lida antes (`settings::get_with` faz as duas coisas sob a
   trava); os testes que mudam o tema, o provedor ou o script ao lado do modelo tomam uma trava só deles (`held`),
   os demais a tomam compartilhada.
   `the_selections_menu_acts_on_all_the_nodes_selected` (`crates/app/tests/canvas.rs`): esperava só 3 quadros
   depois de *Run to them*, e se `c` terminasse antes a fila já estava vazia; agora um breakpoint em `c` segura a
   corrida enquanto a fila é conferida.
   `places_are_kept_with_the_model_and_gone_back_to` (`crates/app/tests/ui.rs`): o campo do nome do menu era
   achado por posição (o primeiro campo abaixo do botão); agora é o da linha do botão Save this place..., e a
   falha diz o aviso do app.
   Vistos depois, na suíte do `ui` (4 threads), passando sozinhos, corrigidos (Unreleased):
   `a_layers_colours_are_given_to_all_similar_layers` (`ui.rs`, o menu do clique direito sem a entrada *Same colours
   for all similar layers*): o aspecto (`Looks`) da camada é calculado por threads e sem ele a linha não tem cores
   no menu; `drawn` agora espera `Looks::busy` e o teste reabre o menu até a entrada existir.
   `a_layer_of_another_coordinate_system_is_drawn_reprojected` (`ui.rs`, a comparação dos pixels `"hidden"`): o mesmo
   aspecto ainda não pronto deixava o mapa sem tiles (não ocupado) e a imagem de referência em branco; `drawn` espera
   os aspectos antes dos tiles.
   `places_are_kept_with_the_model_and_gone_back_to`: o "1" em `sibuyan_lan1duse` é do próprio teste (digita "1" num
   campo de propósito, e o app não toma o dígito como atalho com um campo em foco: `typing` guarda `read_key`); o
   app não tem bug. Com o "Lake" no campo, o botão habilitado e só "Place 1" salvo, o campo "Place 1" na lista é o
   de renomear do submenu *...* da linha do lugar, aberto sobre o botão (abre quando o ponteiro está no retângulo do
   *...*, e cobre o *Save this place...* da linha de baixo): o clique por ponto caiu nele. Não reproduzido em 5
   rodadas; o teste agora clica o *Save* pela ação do accesskit (não por ponto) e a falha diz também as posições do
   menu e o mapa. Causas achadas em 2026-11-15 (suíte do `ui`, 4 threads; antes: `places_are_kept...` falhou em 2 de 11 rodadas):
   um lugar guardado (ou um Plot salvo) é uma edição, o Viewer roda o modelo de novo 250 ms (relógio) depois,
   e o mapa fica 24 pontos mais baixo (uma faixa no alto, 97 para 121); numa rodada lenta a faixa chega no meio do teste e o zoom
   do lugar (ajustado à janela de agora), as células em vista e os cliques por posição não batem com o que o teste calculou.
   Mostrado por uma asserção temporária (o mapa em 121 em 4 de 4 rodadas depois de esperar a corrida). Corrigidos no teste
   (`after_edit`: espera 0,6 s e `drawn`; `places_...` espera o zoom ajustado à janela menor, `a_plot_keeps_the_frame...` aceita
   a caixa ajustada no eixo que cabe e mais larga no outro, `features_selected_by_area...` e `the_layers_as_a_gis_has_them`
   esperam depois de cada edição). Não achei bug do app (não identifiquei qual é a faixa), mas um mapa que muda de altura
   depois de uma edição é um sinal a olhar se alguém reclamar do salto.
   `quality_of_life::the_end_of_a_run_is_told...`: `run_to_the_end` só terminava depois de ver `running()` verdadeiro; uma corrida
   curta que começa e termina dentro de um passo lento (rodada sob carga) nunca era vista, e o teste esperava os 120 s; agora termina
   quando `elapsed` (zerado ao começar, posto ao terminar) está posto e nada corre. Não reproduzido (causa por leitura).
   `web_layers::a_wmts_service...`: o servidor de teste anotava a requisição *depois* de enviar a resposta, e o teste olhava
   `hits()` assim que tinha a resposta; agora anota antes de enviar. Não reproduzido (causa por leitura).
   Visto uma vez, não explicado: `the_runs_are_shown_restored_and_compared` ficou 120 s em "drawing the maps" (a rodada levou
   209 s em vez de 100 s, só esse teste parado); `drawn` agora diz qual dos três (visão do modelo, Viewer, corrida) segurava.

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
     - **Feito** (Unreleased): o projeto como um valor do grafo (`swat_project`, gravado em JSON e congelável),
       `swat.load` (pasta ou zip), `swat.set` (muda um parâmetro como a calibração dura do SWAT+: uma linha no
       `calibration.cal` e, se faltar, no `cal_parms.cal`; pctchg, abschg, absval; camadas do solo e HRUs; os 69
       parâmetros que a porta calibra; condições e plantas recusadas) e `swat.run` lendo o projeto (`project` ou
       `path`).
     - **Feito** (Unreleased): o `watershed` do QSWAT+ (`swat.watershed`, o valor `swat_watershed` com as tabelas
       `gis_*`, `soils_sol*` e as grades das sub-bacias, dos trechos e das HRUs, bit a bit com o landschaft em 18
       casos: os quatro métodos de HRU, limiares, exutórios e `snap`, faixas de declive, ha e percentual, usos
       isentos, `usersoil`, o uso do solo em outra grade; só lat/lon diferem, em até 7e-15 graus pelo PROJ).
     - **Feito** (Unreleased): o projeto a partir das tabelas e do clima (`swat.from_tables`: os arquivos do SWAT+
       Editor 4.0.2, um a um, byte a byte com o `from_tables` do landschaft menos a data da primeira linha de cada
       arquivo, em 6 casos: o vale sintético, com datas, `codes` e nome, com rotações de culturas, com estações de
       períodos e lacunas diferentes e umidade, vento e radiação, com o gerador dado, e o exemplo) e do gerador de
       clima (`swat.wgn`, as estatísticas do WXGEN das séries diárias, bit a bit: a soma em pares do numpy e o
       desvio-padrão do pandas); o clima são tabelas (estações, uma série por variável com `date` ou `year` e `day`);
       o projeto guarda a bacia de onde veio (`watershed` no `swat_project`); a cadeia `swat.watershed` →
       `swat.from_tables` → `swat.run` dá, bit a bit, a rodada do `from_rasters` do landschaft. O `from_rasters` não
       é um operador: é essa cadeia.
     - **Feito** (Unreleased): os resultados no mapa (`swat.hru_map`, `swat.channel_map`, `swat.reach_discharge`, bit a
       bit com o `hru_map`, o `channel_map` e o `reach_discharge` do landschaft em 28 casos: cada `how`, variáveis das
       quatro tabelas de HRU e do canal, a `c_factor` plant, valores dados por HRU e por canal, rotações de culturas, o
       exemplo); leem as tabelas de um `swat.run` do mesmo projeto, ligadas (`hru_wb`, `hru_nb`, `hru_ls`, `hru_pw`,
       `channel_sd`; o passo `aa` para `annual`, `day` para os outros, com a mensagem dizendo o que mudar no
       `swat.run`), ou rodam o projeto elas mesmas; o projeto precisa vir do `swat.from_tables`.
     - **Feito** (Unreleased): o script Python do modelo (`terraflux python`, a ferramenta `python_script`) escreve
       a cadeia toda como `ls.hydro.swat`: `load_txtinout`, `watershed`, `climate` com `from_tables` (que ganha a
       linha `project.watershed = ...`, como o `from_rasters`), `wgn_from_series`, `run` (as tabelas escolhidas no
       passo, as outras desligadas), `hru_map`, `channel_map` e `reach_discharge` (com o `result=` da `swat.run`
       ligada; `nyskip` e `c_factor` numa rodada própria quando nenhuma tabela está ligada); o `swat.set` é uma
       função auxiliar escrita no script (a linha do `calibration.cal` e a do `cal_parms.cal`, como o Terraflux
       as escreve), e as tabelas do clima e dos resultados (ano, dia, objeto) passam ao formato do landschaft e
       voltam por auxiliares. Nenhum dos nove operadores fica como comentário (`--strict` passa). O script do
       template roda no landschaft 1.58.1 e dá as mesmas médias dos mapas e a mesma soma da vazão que o modelo
       (conferidas no teste do template). Fora do script: `swat.wgn` com `monthly` por estação
       (`{"estação": 4}`, que o landschaft não aceita) fica como comentário.
     - Falta: nada do código. O valor `swat_project` (e o `swat_watershed`) é gravado em JSON e congelável; num
       lote distribuído cada tarefa roda o modelo inteiro (a pasta ou o zip de `swat.load` e `swat.run` vão com o
       modelo, Unreleased; um projeto feito dos mapas é refeito no worker). O valor só teria de viajar no motor por
       tiles, que não divide uma bacia: não se aplica.
3. **Tutoriais e templates** de cada um, como os do site do landschaft (a hidrologia do terreno, o SWAT+).
   **Feito** (Unreleased) para o SWAT+: o template "SWAT+ from rasters" (grupo *Surfaces and uncertainty*: um vale
   de 3 km por 3 km desenhado por `nlm` (a `nlm.*` ganha a configuração `crs`), os usos, os solos e dois anos de
   clima inventados em `table.of` (que agora aceita nomes, texto, além de números), `swat.watershed` → `swat.wgn`
   → `swat.from_tables` → dois `swat.set` lendo as entradas `cn2_change` e `awc_change` → duas `swat.run` →
   `swat.hru_map`, `swat.channel_map`, `swat.reach_discharge`), o tutorial *A SWAT+ project from rasters, and its
   results on the map* (`website/content/tutorials/terrain-hydrology/swat-plus-from-rasters.md`, com os limiares
   do fluxo, os métodos de HRU e os parâmetros em tabelas) e o `swat-plus.md` apontando para ele e dizendo de
   `swat.load` e `swat.set`; e o guia `hydrology` do MCP (hidrologia do terreno e SWAT+). Falta, na próxima
   atualização dos dados do site (`refresh_data.py`, `extract_templates.py`): o site ainda não conhece os operadores
   novos (o `ops.json` é de 1.47.0: o build avisa "unknown operator" para todos os `swat.*` novos), e as figuras do
   tutorial (`website/figures/terrain-hydrology/swat-plus-from-rasters.py`) foram desenhadas com o mesmo projeto
   rodado no landschaft (as médias são as do teste do template), porque o binário do cli não estava construído:
   refazê-las com o binário de release. O template guardado em `website/models/templates/` foi salvo pelo
   aplicativo (o `extract_templates.py` só lê os templates escritos em TOML, não os feitos em código).
   Falta a hidrologia do terreno (`hydro.*`), que não tem um template próprio fora os de rios.
4. **O paralelismo do landschaft** (`on=` para as re-soluções da priorização, os SDM e o Monte Carlo): **avaliado**
   (Unreleased), sem código.
   - *O que o `on=` faz no landschaft:* recebe um `ls.helpers()` (`_dist`): o computador de quem chama e os outros
     do grupo de rede local (descoberta por UDP 47810, TCP com TLS, `landschaft worker` em cada um) e reparte as
     tarefas independentes de uma só chamada: `batch` e `mspa_batch`, `parallel.map` e `parallel.tiled` (uma
     ferramenta do landschaft por item ou por bloco do mapa), `simulate.scenarios` e `simulate.calibrate` (as
     buscas em grade e aleatória), `uncertainty.monte_carlo` e `sensitivity.evaluate` (blocos de sorteios, só
     com `ls.parallel.call`), `sdm.fit` (as dobras de cada modelo, os ajustes finais ficam no computador local) e
     as re-soluções da priorização: `calibrate_penalty`, `calibrate_budget`, `pareto_front` (`_solve_all`) e
     `eval_replacement_importance` (uma solução sem cada unidade escolhida); o modelo vai como arrays (CSR) e
     volta o resultado do solver, os argumentos e resultados sempre como dados, nunca como código. O resultado é
     o mesmo de um computador só. O `swat.run_many` do landschaft não tem `on=` (só `n_threads`).
   - *O que o Terraflux já cobre:* o que o `on=` reparte entre computadores como tarefas de **modelos inteiros**: as
     réplicas, os cenários, a calibração, a análise de sensibilidade e os lotes de um modelo do SWAT+ (as entradas
     `$`), com o coordenador, os workers, o aplicativo ("Rodar em mais computadores"), o cluster por SSH e a
     nuvem; o motor por tiles cobre `parallel.tiled`. Isso inclui o Monte Carlo e o `swat.set`: um sorteio ou uma
     rodada é uma tarefa do lote.
   - *O que um nó ganharia:* só o que é paralelo **dentro** de um nó e pesado por tarefa: o `prioritize.calibrate`
     (nove ou mais candidatos, cada um uma solução inteira) e as soluções de substituição e de posto do
     `prioritize.evaluate` (uma por unidade escolhida: centenas), cada uma de minutos num problema grande, com dados
     pequenos (a matriz esparsa) e resultado pequeno. Hoje correm nas threads da máquina (`rayon`, nunca todos os
     núcleos). O `sdm.fit` (≤ 10 dobras, cada uma em segundos: o Terraflux porta os modelos em Rust) e o
     `uncertainty.monte_carlo` (a expressão de cada sorteio é barata e vetorizada) não ganhariam; o `swat.run` é uma
     rodada por nó, e as muitas rodadas já são lotes.
   - *Recomendação:* **não levar o `on=`**. É um mecanismo do `landschaft worker` (outro protocolo e outro pacote em
     cada máquina), e os computadores do usuário do Terraflux rodam workers do Terraflux; o que cobriria o ganho
     real é uma tarefa de **nó** no coordenador que já existe (os arrays do problema como dados do job, os
     resultados de volta), só para o `prioritize.calibrate` e o `prioritize.evaluate`, e só quando um lote ou
     cluster estiver ligado ao modelo. Não vale o trabalho antes de medir: cronometrar o `calibrate` e o
     `evaluate` com replacement num problema grande real (centenas de milhares de unidades) e ver se passam de
     alguns minutos nas threads de uma máquina; se não passam, não fazer.

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
   não instalou dita ao abrir de novo, com o log.
   **Feito** (Unreleased): a assinatura ed25519: o release assina o `SHA256SUMS.txt` (`SHA256SUMS.txt.sig`, a
   chave privada no secret `UPDATE_SIGNING_KEY`) e o app confere com a chave pública embutida antes de confiar nos
   checksums; um release sem assinatura, ou com uma inválida, é recusado e nada é instalado. **Feito**
   (Unreleased): o repositório apt no GitHub Pages (`https://sacridini.github.io/terraflux/apt`, o último `.deb`,
   `InRelease` e `Release.gpg` assinados com a chave GPG do secret `APT_GPG_KEY`, montado pelo `website.yml`); o app
   instalado por ele diz para usar `sudo apt update && sudo apt upgrade`. Os secrets cadastrados em 2026-11-16.
   **Feito** (Unreleased): o zip do Windows. O zip traz um arquivo vazio `portable`; a pasta com ele (e que o
   usuário pode escrever) é trocada arquivo por arquivo pelo PowerShell depois que o app fecha (o que o zip não
   tem fica; um arquivo em uso, como o `terraflux.exe` de um servidor `mcp`, é renomeado `.old` e removido na
   próxima abertura).

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
     Falta oferecê-los no painel Prepare (que não tem hoje um lugar para fontes de dados: o catálogo de dados do
     aplicativo os oferece).
   - **Feito** (Unreleased): os catálogos STAC, o operador `data.stac` (a busca `POST /search` com as páginas, os
     assets lidos por GDAL só na janela da grade e combinados por mediana, média, mínimo, máximo ou o primeiro com
     valor; o Planetary Computer assinado por token SAS), igual ao `download.stac` da biblioteca (testado célula a
     célula contra ela), e o cartão "STAC catalogue" do catálogo de dados com presets do Earth Search e do Planetary
     Computer (Sentinel-2, Landsat, Copernicus DEM, cobertura do solo de 10 m).
5. **O Código Florestal completo.** Há o topo de morro (`surface.hilltop`); faltam as APPs dos rios pela largura (os
   rios do `hydro.streams`), as nascentes, as declividades acima de 45°, as bordas de tabuleiro, e o déficit e o
   excedente de Reserva Legal por imóvel do CAR (o SICAR como entrada), como nos trabalhos do Soares-Filho.
   - **Feito** (0.93.0): `forest_code.app` (as APPs do art. 4º e as faixas do art. 61-A nas áreas consolidadas) e
     `forest_code.reserve` (o balanço da Reserva Legal: arts. 12, 15 e 67), com o template Forest Code: APP and
     Reserva Legal. Falta: as bordas de tabuleiros e chapadas (a linha de ruptura do relevo) e a compensação da
     Reserva Legal entre imóveis (art. 66).
   - **Feito**: o limite do art. 61-B (10 % do imóvel até 2 módulos fiscais, 20 % até 4) em `forest_code.app`: as
     saídas `recompose` e `properties`, mantidas as células mais próximas da água.
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
   - **Feito** (Unreleased): um submodelo (o arquivo de um modelo) se descreve pelo que o arquivo já tem: o painel do
     nó `control.submodel` mostra o `summary` (ou `description`) do modelo, cada entrada com o tipo e o valor (e
     "given" quando o nó a dá) e cada saída com o nó que a dá (`submodel::about`); as configurações escritas
     `"$x"` ou `{ from = "$x" }` dentro do submodelo leem as entradas dele (dadas pelo nó ou o valor do próprio
     submodelo; entram no modelo expandido, que é o que o cache, o `check` e os lotes distribuídos veem), testado em
     `submodel.rs`. Bibliotecas de submodelos (do usuário, do grupo, compartilhadas): o nó do usuário (fase 4) é o
     submodelo da biblioteca, com descrição, portas tipadas, valores próprios, versão e as bibliotecas do item 10.
     Falta: um arquivo de modelo não tem versão própria (o campo seria novo no formato), e as saídas de um
     submodelo no painel não mostram o tipo (o `check` o infere ao expandir).
6. **Plugins além da célula:** uma segunda interface do `plugin.wasm` que receba o mapa inteiro ou uma janela de
   vizinhança, dê várias saídas e trabalhe com tabelas. Opcional: um nó `script.python`/`script.r` para quem os tem
   instalados, marcado como não portátil para os lotes distribuídos (vai contra o "sem Python", mas é a saída de
   emergência que todo usuário avançado procura).
   - **Feito** (Unreleased): as duas interfaces do `plugin.wasm` (`interface = "window"` e `"map"`, a `cell` segue a
     padrão), as várias saídas (`outputs`, `table_outputs`, portas listadas pela assinatura) e as tabelas (colunas
     de entrada e de saída); a ABI está no tutorial dos plugins. **Não feito:** o nó `script.python`/`script.r`.
7. **Depuração do grafo:** desativar ou contornar um nó (bypass) sem apagá-lo; congelar o resultado de um nó caro
   entre sessões; tempo e memória por nó (o perfil da execução); um laço passo a passo, como um breakpoint (o Trace
   já mostra os valores, falta controlar a execução); um nó `check.assert` que falha quando uma condição não vale (a
   demanda atendida, a soma das áreas).
   - **Feito** (1.14.0): o bypass (`bypass = true` no nó: cada saída a primeira entrada do mesmo tipo; B no canvas,
     a caixa apagada), o tempo e a memória de cada nó (`Results.timings`, o corpo de um laço somado nos passos;
     "last run" no painel do nó, `terraflux run --profile`) e o `check.assert` (a expressão do `raster.calc` em
     toda célula com valor, ou do `value.calc`; `warn` só avisa). Congelar o resultado de um nó entre
     sessões (o cache em disco) e o laço passo a passo: **feitos** depois (o `frozen = true`, o `runner::Gate`; ver "A interface: o que falta nas partes já feitas").
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
    feições, porque o GeoPackage grava a hora). O manifesto das execuções do aplicativo: **feito** depois (uma execução inteira do modelo salvo grava `output/manifest.json`, como o `terraflux run`, e entra no diário).
- **Execução incremental entre sessões pela linha de comando**, como o `targets` do R: só o que mudou rodado de
  novo, com o cache persistente.
- **Integração com o ecossistema:** um provedor de Processing para o QGIS (os modelos do Terraflux como algoritmos do
  QGIS; provavelmente o maior multiplicador de adoção) e uma API Python para montar e rodar grafos, além do caminho
  do landschaft.
- **Dados em nuvem:** STAC e o Planetary Computer. **Feito** (Unreleased): `data.stac` e o cartão "STAC catalogue" do
  catálogo de dados (item 4 de "O que ainda falta entre os operadores").
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
  pede e o que falta, "Update to", "changed here since", "Make it a plain group"). As bibliotecas do grupo e
  compartilhadas (o prefixo `my/`, `lab/` do nome), as configurações de um nó do usuário como configurações, a busca
  rápida (`/`) com os nós do usuário, o MCP e o assistente foram feitos depois (item 10 de "A interface: o que falta
  nas partes já feitas", **Feito** (Unreleased)).

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
mostra e grava o nome do arquivo. **Feito** (Unreleased): colunas de texto de verdade, como códigos mais o
dicionário `Table::labels`: `table.read` (CSV, TSV, JSON) as mantém, as tabelas congeladas voltam iguais, e
`table.filter`/`calc`/`sort`/`select`/`aggregate`/`pivot`/`join`, `Table::concat` e `vector.from_table` as entendem.

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
  cima, o contrário (um mapa quando ele escolhe uma banda ou o arquivo tem uma, senão uma pilha). Em vez
  de um nó "Input" de verdade dentro dos contêineres (o formato `$`/`@` do modelo não muda): a caixa de uma
  entrada pode ter cópias no escopo, como os vários *Group Input* do Blender ("Duplicate" no menu ou Ctrl+D; guardadas
  em `places` como `escopo|referência#2`, `#3`...; cada nó lê da cópia mais próxima da sua porta, sem atribuição
  guardada; apagar uma cópia tira só o lugar, apagar a última desconecta quem lia, como antes).
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
  "Large maps" também. Falta: a lista das falhas num painel (fase 2). (O nome do arquivo na coluna `layer` já
  existe: texto nas tabelas feito.)
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
   (`methods::method_of`, `Reference::cite`) e dicas escritas à mão (`TIPS`, `crates/ops/src/tips.rs`): 88 operadores
   (Unreleased): LUCC (calibrações, simulações, validações), MSPA e SPA, métricas, SDM, priorização, circuitos,
   custos, conectividade em grafo, hidrologia, serviços (InVEST), agentes, REDD+, preparo de dados (`raster.*`,
   `vector.*`), análise de mudança (`change.*`), acurácia (`accuracy.*`) e sensibilidade (`sensitivity.*`). Teste:
   toda dica é de um operador do registro.
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
7. **Avaliar o efeito.** **Feito** (Unreleased): o `eval.rs` tem 9 tarefas (as duas últimas: métricas pedidas em
   português, *densidade de bordas*, e o método de um modelo Dinamica revisado sem mudá-lo, em que a resposta tem de
   dizer que falta a validação). O modo `plain` (`eval::Mode`, `tools::set_plain`) roda as mesmas tarefas SEM a
   ajuda nova (sem os conselhos de método do `check_model`/`run_model`, sem as dicas e o método do
   `describe_operator`, com as instruções reduzidas a um parágrafo: `tools::PLAIN_INSTRUCTIONS`); `keep` guarda o
   histórico (modelo, modo, versão, data, por tarefa passou/falhou, segundos, ferramentas) e `eval::compare` dá a
   tabela de dois registros. O servidor da HU é pedido no máximo 12 vezes por minuto (um pedido a cada 5 s) e nada
   em paralelo. O teste `#[ignore] eval::tests::hu_berlin_does_the_tasks` roda os dois modos com o Qwen da HU
   (pasta temporária própria) e imprime a comparação. **Resultado de 2026-11-16** (Qwen `llm3` da HU, Terraflux
   1.53.0, uma rodada de cada, 14 min no total):

   | tarefa | sem a ajuda (plain) | com a ajuda (full) |
   |---|---|---|
   | uma paisagem neutra | passou, 36 s, 7 ferramentas | passou, 56 s, 10 |
   | um limiar | falhou, 30 s, 5 | falhou, 54 s, 10 |
   | um template mudado | passou, 31 s, 5 | passou, 29 s, 5 |
   | um modelo consertado | falhou, 19 s, 3 | passou, 20 s, 3 |
   | uma explicação | passou, 5 s, 0 | passou, 5 s, 0 |
   | métricas de paisagem | passou, 53 s, 9 | passou, 51 s, 10 |
   | um renome | passou, 7 s, 1 | passou, 8 s, 1 |
   | um laço | falhou, 201 s, 40 | falhou, 200 s, 40 |
   | um método revisado | passou, 5 s, 0 | passou, 6 s, 0 |
   | **total** | **6 de 9, 386 s, 70 ferramentas** | **7 de 9, 431 s, 79 ferramentas** |

   Leitura: uma rodada de um modelo pequeno não prova nada; a diferença (1 tarefa) é dentro do ruído, e as falhas
   comuns (o limiar: nenhum `raster.calc` deu o mapa do campo; o laço: estourou as 40 rodadas de ferramentas) não
   dependem da ajuda. Para valer, repetir 3 a 5 vezes e com um modelo grande (sem chave aqui):
   `TERRAFLUX_EVAL_PRESET=5 TERRAFLUX_LLM_KEY=<chave> cargo test -p terraflux-app --lib eval::tests::hu_berlin_does_the_tasks -- --ignored --nocapture --test-threads=1`
   (preset 4 = OpenAI, com `TERRAFLUX_EVAL_MODEL=<modelo>`; 5 = Claude; a chave também pode estar no keyring do app,
   em Assistente > servidor) e `TERRAFLUX_EVAL_ONLY=plain` ou `full` para um modo por vez. A comparação sai no
   fim, e o histórico fica em `%TEMP%/terraflux-eval-hu/assistant-evaluations.json`.

Falta:

- **Elicitation** (MCP 2025-06-18): **Feito** (as classes de um mapa sem legenda no `set_input`, `Server::name_classes`
  em `crates/cli/src/mcp.rs`; os arquivos do usuário depois do `new_model` com template, `Server::ask_files`,
  Unreleased). O servidor manda `elicitation/create` ao cliente pela mesma saída, guarda os pedidos em aberto
  (`Client::waiting`) e entrega a resposta à thread que espera; só quando o cliente declara `elicitation` no
  `initialize`. Falta: "qual o período da calibração?" (as datas dos mapas de uma série, para as regras de
  método) e testar com clientes reais (Claude Code, Claude Desktop, Cursor) a forma do formulário.
- **Mais regras de método** (`advice.rs`): **Feito** em grande parte: `markov.fit` com `steps` incoerente com o
  intervalo das datas, drivers de `woe.fit` sem nenhuma distância, resistência com zeros ou negativos em `circuit.*`
  e `cost.transition`, `metrics.*` com classes demais, priorização sem `prioritize.evaluate`, SDM sem limpeza nem
  thinning dos registros: todas já estão em `advice.rs`, cada uma com teste e sem warning nos templates.
- **Os conselhos no app**: hoje chegam ao assistente; mostrá-los também no painel Problems (com o nível "advice",
  abaixo dos warnings de dados) e no relatório, na seção de checagens.
- **Mais dicas por operador**: **Feito** em grande parte: as famílias `conn.*`, `hydro.*`, `services.*`,
  `agents.*` e REDD+ já têm dica, e 39 operadores a mais ganharam (Unreleased; 88 no total). O campo `advice` em
  `OpSignature` já existe (um operador pode dar a sua; `tips.rs` completa o registro). Falta: `sdm.*` além de
  `maxnet`/`background`/`threshold`/`evaluate`/`mess`, `vector.*` e `raster.*` menos usados, `distance.*`,
  `fragmentation.*`.
- **`guide` para os modelos pequenos**: o modo de poucas ferramentas (`tools::FEW`) não tem o `guide`; ver na
  avaliação se acrescentá-lo ajuda o Qwen ou só aumenta o pedido.
- **Saída estruturada** (`outputSchema` e `structuredContent`, MCP 2025-06-18) no `check_model` (erros, warnings e
  conselhos como lista, com o nó e o guia) e no `run_model` (os arquivos escritos), para clientes que mostram ou
  usam os campos; o texto continua para os outros.
- **Guias em português**: os textos dos guias estão em inglês (como as instruções e os operadores); um
  `guide(topic, language: pt)` a partir de `pt.rs` para o assistente do app, que responde na língua do usuário.

## O Viewer para análise: transições, áreas, regiões, perfis, séries e vistas

Pedido de 2026-11-04 (um commit por item; release só quando pedido). O Viewer já mostra e compara mapas; faltava
responder no próprio Viewer as perguntas de quem estuda mudança de uso, sem montar nós.

1. **A matriz de transições** na diferença de dois mapas de classes: origem × destino em hectares, km², % da
   paisagem, % da classe de origem ou células; ganhos, perdas, saldo e troca por classe em barras; um clique numa
   casa destaca só aquela transição no mapa.
   **Feito** (Unreleased): `difference::Matrix` (as classes de cada mapa, as células e a área de cada par, a área
   de cada célula pela linha da grade, `measure::cell_area`: geodésica numa grade em graus, nenhuma sem sistema de
   coordenadas) e `Matrix::budgets` (antes, depois, ganho, perda, `net`, `swap` = 2·min(ganho, perda), como em
   Pontius). `difference::focused` refaz a diferença com um par destacado (1: o par, 2: as outras mudanças, 0: sem
   mudança em cinza claro), numa thread (`Difference::focus`, `focused`, `shown`). A janela
   (`transition_view.rs`): abas *Matrix*, *Gains and losses* e *Flows* (um Sankey de antes para depois, cada fluxo
   da cor da classe de origem; o fluxo clicado também destaca), a unidade, *Copy* (TSV) e *Save CSV*. Corrigido
   no caminho: aberta direto em *Difference* (sem o mapa desenhado antes), a vista da diferença herdava zoom 0 e
   não aparecia. Testes: `the_matrix_of_every_transition_with_gains_and_losses`, `a_transition_singled_out`,
   `amounts_in_each_unit`, `the_matrix_as_text`, `the_transition_matrix_of_a_difference` (UI).
2. **A área de cada classe** na legenda: hectares, km² e % da paisagem, do mapa inteiro, da vista atual ou de uma
   região.
3. **Estatísticas de uma região**: um polígono desenhado (a linha da medição, de três pontos) ou as feições
   selecionadas na tabela de atributos; classes: a área de cada uma dentro dela; valores: média, mínimo, máximo e
   soma.
   **2 e 3 feitos** (Unreleased): `region.rs`. `region::Part` (o mapa, uma caixa de células: a vista, ou polígonos
   em células da grade; a célula entra pelo centro, par-ímpar, então buracos e multipolígonos valem) e
   `region::of` (classes: células e área de cada uma; valores: células, área, média, mínimo, máximo, soma, desvio),
   lido a cada `histogram::sample_step` quando a caixa passa de ~4 milhões de células, cada célula amostrada
   valendo as em volta; a área pela linha (`measure::cell_area`). `region::Jobs<T>`: o cache genérico de trabalhos
   numa thread por mapa e chave (as estatísticas, os perfis, as áreas dos passos). Na barra do Viewer, sob a
   legenda: *map*, *view* (a vista só depois de parada ~0,3 s, `settled_view`), *polygon* (a linha da medição de
   três pontos), *selected* (os polígonos selecionados na tabela de atributos, `selected_rings`, pela camada
   preparada na grade do mapa); ha, km² ou células e a parte; *Copy*. Testes: `the_area_of_each_class`,
   `a_polygon_takes_the_cells_whose_centres_are_in`, `values_mean_extremes_and_sum`,
   `the_area_of_the_classes_in_a_region_and_a_profile` (UI).
4. **Perfil ao longo de uma linha**: a linha da medição amostrada, os valores num gráfico pela distância.
   **Feito** (Unreleased): `profile.rs`. Pontos a cada ~meia célula (no máximo 2 000) pela linha da medição, a
   distância em metros pela `measure::cells_apart` (geodésica em graus); um raster em disco lido de uma vez pela
   caixa da linha quando cabe, senão célula a célula. Valores: gráfico (egui_plot) pela distância, os vértices
   marcados; classes: a faixa das classes cruzadas e o comprimento em cada (`class_lengths`). O ponto sob o ponteiro
   no gráfico vira a cruz (`ghost`) no mapa. Testes: `values_along_a_line`, `the_length_in_each_class`.
5. **A série no tempo** dos modelos com passos: a área de cada classe ao longo dos passos, ao lado do Play; o
   Sankey do primeiro ao último passo.
   **Feito** (Unreleased): `steps_section` (builder.rs): as áreas de cada passo (`region::of` de cada mapa, numa
   thread, `Session::steps_areas`), uma linha por classe na cor dela, o passo mostrado marcado, o clique leva ao
   passo; *Copy*; o botão "*primeiro* to *último*..." abre a janela das transições (`transition_view::window` sem
   foco) com a matriz de `difference::make` do primeiro ao último passo (`Session::steps_matrix`), nos fluxos
   primeiro. Teste: `the_area_of_the_classes_along_the_steps_and_the_flows_from_the_first` (UI).
   **O Sankey de todos os passos feito** (Unreleased): `transition_view::layers` (as classes de todos os passos,
   uma linha por classe em cada coluna, altura zero onde ausente, e as faixas de cada par) e `sankey_chain` (uma
   coluna por passo, os nomes dos passos embaixo); uma matriz por par de passos, cada uma num trabalho
   (`Session::steps_pairs`), "computing..." até todas chegarem. Escolha *First and last* / *Every step*
   (`State::every`; padrão: todos os passos de 3 a 8, o primeiro e o último acima, no máximo 12 passos); *Copy* e
   *Save CSV* com um bloco por par (`from_step`, `to_step`). Testes: `two_steps_are_the_flows_of_their_matrix`,
   `three_steps_add_up_column_by_column`, `a_class_absent_at_a_step_keeps_its_row`,
   `the_flows_of_every_step_as_text`, `the_flows_of_every_step_of_a_series` (UI).
6. **Vistas salvas**: o enquadramento guardado no Plot, e a figura, o export e o Document no mesmo recorte.
   **Feito** (Unreleased, por um sub-agente): `Plot.view: Option<PlotView { extent: [oeste, sul, leste, norte] }>`
   nas coordenadas do mapa do Plot (não depende do tamanho da janela; omitido quando não há). `frame.rs`: `cells_of`
   e `extent_of` (também em grades rotacionadas) e o recorte da fonte numa grade menor (`crop_map`, `crop_source`,
   `crop_panel`, `crop_value`; um raster em disco vira um `Map` só da janela), então vetores, camadas, coordenadas,
   escala e inset seguem `source.grid()` sem mudança nos desenhistas. Usado pela figura e por *Export the view* (o
   Plot aberto), `plot_export` (`terraflux plot`, `export_plot`), o Document e o relatório. No menu Plots: *Keep
   this view in the plot* / *Update the saved view*, *Go to the saved view*, *Forget the saved view*; abrir o Plot
   leva o mapa ao quadro (`Session::plot_frame`, `MapView::show_extent`). `set_plot` aceita `view`; o check acusa
   uma extensão vazia. Testes: os de `frame`, `a_saved_view_draws_only_its_part`,
   `a_figure_shows_the_saved_view_of_its_plot`, `a_figure_carries_the_saved_view_of_its_plot`,
   `a_plot_keeps_its_view`, `a_plot_keeps_the_frame_of_its_map` (UI). Falta: *Save as a new plot* não grava a vista;
   `set_plot` sem `view` apaga a vista (o Plot é trocado inteiro); ~~o recorte de um raster grande em disco é lido na
   thread da interface ao abrir a figura~~ (feito 2026-11-14: `frame::open` corta numa thread e `frame::window`
   mostra "reading the map..."; o Document corta num job por (nó, porta, caixa); o relatório HTML/PDF ainda corta na
   thread da interface, dentro da geração inteira); Python e QGIS não leem a vista; a figura do Document ainda sem vetores.
7. **O painel de camadas como num GIS** (pedido de 2026-11-04): arrastar para reordenar (não funcionava), ligar e
   desligar fácil, trocar a paleta clicando na própria paleta, mínimo e máximo do histograma, *Add a layer* que
   funcione com camadas já presentes, o aspecto de um GIS comum (QGIS).
   **Feito** (Unreleased): `layers_panel.rs` (submódulo do builder). Uma barra (*LAYERS*, ➕ com as saídas da
   execução e *A file from disk...*, 🗑, ⏷, ⏶, 🔍 e 👁 sobre a camada selecionada) e uma lista emoldurada, a camada
   de cima primeiro: caixa de visibilidade, seta que dobra a legenda e os ajustes, símbolo (a rampa, as primeiras
   classes, pontos/linha/polígono na cor do estilo, ± de uma diferença), nome cortado e ⋯. Clique seleciona (um
   mapa vai para a direita), duplo clique dá zoom, botão direito e ⋯ abrem o menu (zoom, tabela de atributos,
   cores, só esta camada, todas, topo/subir/descer/fundo, remover). O arrasto é próprio (a linha onde a camada
   cai entre duas, o nome junto do ponteiro; `move_to_slot`), não mais o `dnd` do egui (soltar sobre a linha de
   outra, que não funcionava na prática). *Add a layer* falhava em silêncio: a camada nova ia para baixo do mapa mostrado (fora das camadas,
   desenhado por cima de todas); agora o mapa mostrado é fixado e a nova vai por cima; arquivos do painel Files
   soltos sobre a lista também viram camadas. A caixa do mapa mostrado agora o esconde e a opacidade vale para ele
   (`MapView::opacity`). A paleta: a barra da legenda (à direita e em cada camada) abre as rampas desenhadas, *by
   what they are* e *logarithmic*; a seção *Colours* saiu. O histograma (`histogram::show_ranged`): duas linhas
   arrastáveis, *colours from ... to ...*, *2% to 98%* e *least to most*; a faixa por saída em `Session::ranges`,
   guardada no Plot (`range = [a, b]` no PlotLayer, `set_plot` aceita), usada no export do Plot, no Document e no
   QGIS (`builder::ranged`, `range_of`). As camadas agora são desenhadas na paleta e na legenda do modelo como
   quando mostradas (`default_palette`, `legend_of`), como o símbolo. Testes: `a_row_moved_among_the_others`,
   `the_layers_as_a_gis_has_them` (UI), `maps_stack_and_cells_show_their_series` (UI, refeito para a barra, o menu
   e o arrasto). **Feito** (Unreleased): grupos de camadas como no QGIS, um nível (`Plot.groups`, `PlotGroup`,
   `PlotLayer.group`; `Group` e `drawn_layers` no builder, `tidy` e a linha do grupo em `layers_panel.rs`: seta,
   caixa, opacidade que multiplica a das camadas, renomear, *Ungroup*; arrastar uma camada sobre a linha do grupo
   a põe dentro, arrastar para fora a tira; grupo escondido não desenha); renomear uma camada sem renomear o nó
   (`PlotLayer.name`, *Rename...* no menu ou duplo clique no nome; vale na lista, na legenda, no export do Plot e
   nas vistas lado a lado); selecionar várias camadas (Ctrl alterna, Shift pega o intervalo; a caixa de uma vale
   para todas, o arrasto leva o bloco, *Group* e remover agem na seleção). As edições da lista vivem na sessão
   (como a visibilidade e a opacidade): entram no modelo, e no desfazer, quando o Plot é salvo. Testes:
   `a_layer_is_renamed_without_renaming_its_node`, `layers_in_groups`, `several_layers_are_selected` (UI),
   `the_layers_of_a_group_are_together`, `a_block_of_rows_moved_among_the_others`. Falta: a faixa de cores nas
   vistas lado a lado com camadas próprias.
8. **Vários arquivos de uma vez, séries e stacks** (pedido de 2026-11-04): arrastar vários rasters/vetores da pasta
   criava os nós, mas o Viewer só mostrava o último (o último selecionado).
   **Feito** (Unreleased): `builder::as_layers` (a parte de `drop_on_viewer` que fixa as camadas, os mapas e depois
   as feições, e o mapa mostrado sob elas); vários arquivos soltos fora do Viewer (do sistema, `take_dropped`, ou
   do painel Files no canvas) viram todos camadas, sem trazer o Viewer para frente; o Viewer (`viewer_pane`) aceita
   o arraste do painel Files; uma pasta (do sistema ou do painel Files) vira `raster.read` da pasta, uma stack com
   *Play* (`add_file`; uma pasta sem raster é recusada). Testes: `files_dropped_on_the_viewer_become_layers_of_it`
   (estendido: vários fora do Viewer, uma pasta), `files_of_the_files_pane_let_go_over_the_viewer_are_its_layers`,
   `files_become_nodes_and_rasters_join_a_stack_by_a_connection` (a pasta agora é um nó).
   **Feito também** (Unreleased, pedido em seguida): vários rasters soltos de uma vez como uma stack só, por escolha
   depois de soltar (`series_drop.rs`): `as_layers` guarda os nós (`Session::dropped_rasters`, dois ou mais
   `raster.read`) e o painel de camadas oferece *As one stack (a time series)* ou *Keep them apart*. Numa pasta,
   `series_drop::pattern_of` acha o padrão (o começo e o fim comuns dos nomes em volta de `*`) que nomeia
   exatamente esses arquivos (conferido com `files::files_of`; se nomeia outros, não serve) e um `raster.read` dele
   substitui os nós (os lidos por outro nó ficam); em pastas diferentes, um `stack.of` alimentado por eles. A stack
   vira a camada no lugar das deles e é mostrada; Ctrl+Z desfaz (é uma edição). Testes:
   `a_pattern_names_exactly_the_files_dropped`, `rasters_dropped_at_once_joined_as_one_stack` (UI).
   **Feito também** (Unreleased, pedido em seguida): os nós que leem um arquivo levam o nome dele
   (`editor::id_of_file`, aplicado em `builder::add_at` a `raster.read`, `vector.read` e `table.read` com `path`;
   `editor::add_node_named`): `landuse_1997`, a pasta pelo nome, o padrão pelas palavras (`landuse`), sem acentos,
   `map_` antes de um dígito. Os mapas de uma stack pelo ano (`builder::by_year` em `maps_of`, com
   `advice::year_in`): quando cada nome tem um ano, diferente, e o resto do nome é igual; só a exibição (o nome da
   camada na stack não muda: `{name}` do `raster.write` segue `landuse_1997`). Uma pasta solta (`builder::add_path`)
   traz também seus vetores, um `vector.read` cada (não `.json`), camadas sobre a stack. Testes:
   `a_node_that_reads_a_file_is_named_by_it`, `the_maps_of_a_stack_named_by_their_years`, e os de UI
   `files_dropped_on_the_viewer_become_layers_of_it` (pasta com um vetor) e
   `rasters_dropped_at_once_joined_as_one_stack` (nomes e o ano sob *Play*); os testes de UI que esperavam
   `raster_read` passam a usar o nome do arquivo.
   **Feito também** (Unreleased, pedido em seguida): o ano no visualizador da simulação (`app.rs`, as bandas de uma
   camada de stack, por `builder::by_year`). O `vector.read` de uma pasta ou de um padrão (`vector::vector_files_of`,
   `vector::merge_vectors`): as feições de todos os arquivos numa camada, no sistema de coordenadas do primeiro (os
   outros reprojetados por `reproject_vector`), a união dos atributos (NaN ou texto vazio onde faltam; número e
   texto viram texto) e `file` (`source_file` se já houver um `file`); `.json` fora numa pasta. A pasta solta
   (`builder::add_path`) vira um só `vector.read` dela (do arquivo, se for um), `<pasta>_features` ao lado da
   stack. O cache do executor (`files_read`) olha cada arquivo de uma pasta ou do padrão (antes só a pasta: um
   arquivo mudado dentro dela não refazia a leitura, também do `raster.read`). Testes:
   `vector::the_vectors_of_a_folder_are_one_layer` (ops) e `files_dropped_on_the_viewer_become_layers_of_it` (UI,
   dois gpkg na pasta). Falta: o export Python do `vector.read` de uma pasta (o `ls.load_vector` do landschaft lê um
   arquivo só).

## O canvas: qualidade de vida ao montar modelos

Pedido de 2026-11-04: "esses detalhes de qualidade de vida ainda são muito necessários no terraflux". Os gestos
que um modelo de verdade pede muitas vezes, hoje feitos um nó (ou uma conexão) por vez. Um commit por item; release
só quando pedido.

Retomado em 2026-11-06: "pode implementar todas as ideias listadas uma a uma. Pode ir fazendo commit e passando para
a próxima de forma automática." A ordem: 4, 5, 12, 14, 7, 6, 9, 11, 16, 17, depois 8, 10, 13, 15, 18, 19; por fim as
sobras pequenas do Viewer ("O Viewer como um SIG"): *A URL...* também no cabeçalho do Viewer, *Go to* com zoom e a
marca nas vistas ao lado, a seleção guardada no Plot com a tabela fechada, o blend das camadas reprojetadas.

1. **Ligar os nós selecionados de uma vez.** **Feito** (Unreleased): a conexão puxada da saída de um nó
   selecionado leva os outros selecionados junto (os fios deles desenhados, "3 connections" no ponteiro); solta num
   nó que aceita qualquer número (`stack.of`), cada um vira uma entrada nova, na ordem em que estão no canvas;
   solta em outro nó, preenchem as entradas livres que servem, em ordem. O menu do nó faz o mesmo (*Connect the 3
   nodes selected here*, todas as saídas de um grupo selecionado). Um passo de desfazer
   (`editor::connect_many`, `Scene::along`, `Scene::links_into`). Teste: `the_nodes_selected_are_connected_together`.
2. **Um grupo que se conecta ao resto.** **Feito** (Unreleased): o grupo dá como saídas os resultados dos seus
   últimos nós (os que nenhum outro do grupo lê): rasters agrupados sem ninguém que os lesse, o fim de uma cadeia
   (`editor::last_outputs`). Um grupo já feito os ganha pelo menu, *Give it the outputs of its last nodes*
   (`editor::expose_last`). Teste: `a_group_gives_the_results_of_its_last_nodes`. **Saída por arrastar feita** (Unreleased): dentro do grupo aberto, um fio solto na faixa "new output of the group" (à direita
   do canvas, desenhada enquanto se arrasta) faz da porta uma saída do grupo (`editor::expose_output`, nome livre: a porta, `porta_2`...;
   a mesma porta de novo não duplica), e as saídas do grupo aparecem como caixas à direita dos nós. Teste:
   `a_connection_let_go_on_the_edge_of_a_group_makes_an_output`. **Renomear e reordenar as saídas feito**
   (Unreleased): no painel do grupo cada saída tem o nome para editar (Enter renomeia: `editor::rename_output`
   troca `grupo.velho` em tudo que lê, saídas do modelo, Plots e contêiner, validando com `valid_port`, num passo
   de desfazer) e setas ↑ ↓ (`editor::move_output`). A ordem fica em `Body.order`, lista opcional ao lado de
   `outputs` (que continua um mapa por nome): arquivos antigos leem igual, versões antigas ignoram o campo.
   Testes: `the_outputs_of_a_group_are_renamed_and_ordered`, `a_groups_outputs_are_renamed_and_ordered_in_its_panel`.
   Falta: criar uma *entrada* do grupo arrastando um fio da borda esquerda.
3. **O grupo com cara de grupo, e a entrada e a saída animadas.** Uma caixa que se lê como "vários nós dentro"
   (cartões empilhados, o número de nós, um ícone) e, ao entrar ou sair, o zoom para dentro da caixa e de volta.
   **Feito** (Unreleased): dois cartões atrás da caixa, a borda e o cabeçalho na cor de destaque, um ícone e
   `group · 5 nodes` na linha do operador (de zoom 0.6 para cima). Ao entrar, a câmera do corpo parte da caixa do
   grupo e se abre até o canvas; ao sair, a cena de fora encolhe até a caixa (cerca de 0.3 s, ease-out cúbico, um
   véu e uma moldura que acompanham; nada com `animation_time` 0). `editor_view::Transit`. Teste:
   `a_group_looks_like_one_and_going_in_and_out_is_a_zoom`. Falta: ver no tema claro; uma foto da cena de fora
   (hoje um véu) para que ela se apague em vez de cortar; uma silhueta dos nós de dentro.
4. **Soltar um fio no vazio abre a busca de operadores**, só os que aceitam aquele tipo, e o nó escolhido nasce ali,
   já ligado (como no Blender, Houdini e Unreal).
   **Feito** (Unreleased, por um sub-agente): a soltura no vazio (`editor_view`, `Drag::Connect` com `carried`: uma
   conexão arrancada de uma entrada só se desliga) abre o `Quick` com uma `Wire` (as saídas arrastadas, os tipos, o
   ponto); `quick_add` filtra por `editor::takes` (`Type::fits`), o tipo exato primeiro; `builder::add_wired` adiciona,
   liga (`editor::wire_links`, `connect_many`) e seleciona num passo de desfazer. Testes:
   `a_wire_let_go_on_empty_canvas_asks_for_an_operator_that_takes_it`,
   `wires_dragged_together_go_into_the_operator_chosen`, `a_wire_let_go_inside_a_group_adds_the_node_to_its_body`.
   **Entrada livre feita** (Unreleased): arrastar de uma entrada sem ligação desenha o fio da porta até o ponteiro
   (`Drag::Feed`); solto numa saída que serve liga, solto no vazio abre o `Quick` com um `Feed` só com os operadores que dão o tipo
   (`editor::gives`), o escolhido nasce ali (a caixa termina no ponto) com a saída que serve ligada (`builder::add_fed`, `editor::feeding_port`);
   Esc ou soltar fora cancela. Teste: `a_free_inputs_wire_dragged_to_an_output_or_the_empty_canvas`.
   Falta: o nó nasce pelo canto, não pela porta; o filtro só pelo tipo do primeiro fio (e, no caminho direto, só pelo tipo).
5. **Inserir um nó num fio**: arrastar um nó sobre uma conexão e soltar o coloca no meio (a entrada e a saída que
   servem); alt+arrastar o tira de lá ligando de novo o que ele ligava.
   **Feito** (Unreleased, por um sub-agente): `editor::insert_between` e `editor::lift_out`; no canvas
   `Scene::insertion` (o fio cruzado mais perto do ponteiro, os tipos que servem, sem ciclo: `Scene::reaches`) e
   `Scene::lifting`, as ações `Insert` e `Lift` na soltura de um nó só (`Drag::Move` lembra o alt do começo), o fio
   alvo em destaque. Testes: `a_node_let_go_over_a_connection_goes_in_the_middle`,
   `a_node_dragged_with_alt_is_lifted_out_of_its_connections`. Falta: o desfazer volta as ligações mas não o
   movimento (o passo do arrasto já fechado, `undo.rs`); a inserção de vários nós. Teste da mensagem de corte: `a_lift_that_cuts_a_connection_says_so`.
6. **O menu da seleção**: botão direito com vários nós selecionados dá apagar, duplicar, desligar tudo, agrupar,
   rodar até eles, congelar, alinhar; hoje o menu é de um nó só.
   **Feito** (Unreleased, por um sub-agente): `editor_view::selection_menu` (as `Action`s `DeleteChosen`,
   `DuplicateChosen`, `DisconnectChosen`, `FreezeChosen`, `BypassChosen`, `RunToChosen`; os motivos em
   `EditorState::no_freeze`/`no_run`), `builder::delete_chosen`/`freeze_chosen`, `editor::disconnect_all` (só as
   `Ref`), `Session::start_to_many` e `run_queue` (o motor para num alvo: os últimos um após o outro). Teste:
   `the_selections_menu_acts_on_all_the_nodes_selected`. Falta: alinhar (o item 8); o clique direito num nó fora
   da seleção não o seleciona (o item 1 depende disso); o Esc solta a seleção antes de fechar o menu; uma edição
   no meio do "Run to them" não esvazia a fila.
7. **Mudar uma configuração em vários nós de uma vez**: com nós do mesmo operador selecionados, o painel Node mostra
   as configurações em comum ("vários valores" onde diferem) e muda todas de uma vez (por exemplo o `nodata` de 20
   `raster.read`).
   **Feito** (Unreleased, por um sub-agente): `editor::set_param_many` (em `guarded`: recusado num, em nenhum);
   `node_view::many_panel` e `many_setting` (o valor comum ou "several values: …", escolhas e booleanos num ComboBox,
   o texto confirmado ao perder o foco; operadores misturados listados, um clique seleciona um). Teste:
   `a_setting_is_changed_in_the_nodes_selected_at_once`. O campo de texto tem rótulo acessível (`labelled_by`) e o teste digita nele de verdade. Falta: as listas, regras e scripts editados como no painel de um nó; uma nota para todos; o painel de um nó
   ainda aceita um valor que o checker recusaria (o de vários recusa).
8. **Alinhar e distribuir** os selecionados (à esquerda, centro, em coluna com o mesmo espaço), além do *Arrange* do
   modelo inteiro.
   **Feito** (Unreleased, por um sub-agente): o submenu *Align* do menu da seleção; `editor_view::Align` (dez modos)
   e `aligned` (pura, os deslocamentos), `EditorState::rects_of` (as caixas reais), `builder::align_chosen` (pelo
   `editor::move_node`, um passo de desfazer). Teste: `the_selections_menu_aligns_and_distributes`.
   **Atalhos feitos** (Unreleased): alt+shift+setas (bordas), C e M (centros), H e V (distribuir), ações `canvas_align_*` e
   `canvas_spread_*` do `keymap` (remapeáveis), lidas em `builder::keys` antes das setas simples (o egui ignora alt e shift
   a mais no `consume_key`); teste `the_align_keys_do_what_the_menu_does`. **Coluna e linha feitas** (Unreleased): alt+shift+K
   (coluna) e alt+shift+R (linha), `canvas_stack_column` e `canvas_stack_row` em `ALIGN_KEYS`, remapeáveis, no menu e no F1;
   teste estendido. Falta: os
   espaços de coluna e linha (20 e 60) são uma escolha, não uma constante comum com o *Arrange*.
   **Tema claro dos grupos feito** (Unreleased): `editor_view::group_look` (borda e cabeçalho a 3:1 do canvas, título a
   4.5:1, o ícone do grupo a 4.5:1), `notes::inks` para notas e molduras; o tema escuro intacto (testes
   `a_group_reads_in_the_light_themes`, `a_group_in_the_dark_themes_is_as_it_was`).
9. **Duplicar arrastando** (ctrl+arrastar) e **duplicar com as entradas** (ctrl+shift+D: a cópia lê o mesmo que o
   original), útil para um ramo de análise paralelo. **Feito** (Unreleased): ctrl+arrastar copia o nó (ou a
   seleção, arrastada por um nó dela) para onde é solto, só contornos seguindo o ponteiro, num passo de desfazer;
   Esc cancela; ctrl+clique continua alternando a seleção. O ctrl+D já duplicava lendo as mesmas entradas, então
   não entrou um ctrl+shift+D (seria o mesmo); a ajuda do *Duplicate* nos menus diz isso. Falta: inserção em fio
   e alt (lift) no ctrl+arrastar; uma dica de "copiando" no ponteiro.
10. **Desligar todas as entradas** de um nó (ou da seleção) pelo menu, e **trocar dois fios** de lugar arrastando
    um sobre o outro.
    **Feito** (Unreleased, por um sub-agente): *Disconnect all inputs* no menu de um nó (`Action::DisconnectAll`);
    a troca: o fio pego de uma entrada (`EditorState.carried`) solto noutra ligada, `Scene::swap_with` (os tipos nos
    dois sentidos), `editor::swap_wires` (em `guarded`), `History::join` (pegar e trocar num passo). Teste:
    `a_nodes_menu_disconnects_all_and_wires_swap`. Falta: com tipos que só servem num sentido o fio substitui o do
    alvo, como antes (o outro nó perde a ligação, um desfazer volta); nas portas dinâmicas (`raster.calc`) as
    entradas se reorganizam quando o fio é pego.
11. **Trocar o arquivo de um `raster.read`** pelo menu ou soltando um arquivo em cima do nó, mantendo os fios (hoje
    se apaga e refaz).
    **Feito** (Unreleased, por um sub-agente): `Action::ChangeFile`/`SetFile`, *Change file...* no menu dos
    `raster.read`/`vector.read`/`table.read`; `builder::change_file` (pelo `set_param_many`, um passo de desfazer),
    `read_op_of`/`fits_read` (a detecção do `add_file`), `file_dialog_for`; `EditorState::node_at` e
    `drawn_rect_of` (pela câmera desenhada, `EditorState::drawn`); em `app.rs::take_dropped` o nó acende e o
    arquivo solto troca o dele. Testes: `a_read_nodes_file_is_changed_keeping_its_links`,
    `a_file_dropped_on_a_read_node_replaces_its_file`. O brilho do hover é testado (`TerrafluxApp::drop_lit`: acende com o arquivo em cima, apaga ao sair do nó ou com outro tipo de arquivo). Falta: o id seguindo o arquivo (o item 15); o caminho guardado pelo `value_of` (um nome sem extensão que pareça número seria lido
    como número).
12. **Buscar um nó do modelo** (ctrl+F no canvas): pelo id, operador ou arquivo, a câmera vai até ele (dentro de
    grupos também, abrindo o grupo).
    **Feito** (Unreleased, por um sub-agente): `find.rs` (`walk` em toda profundidade, `hits` pelo id, operador,
    `note` e arquivos das configurações, até 12; `go`: o `editor.path`, a seleção e `EditorState::centre_on`, o zoom
    entre 0,7 e 3; `show`, a caixa no topo do canvas); Ctrl+F em `builder::keys`; o *Go to* do Ctrl+K para nós
    aninhados (`Do::GoTo(Vec<String>)`). Teste: `a_node_of_the_model_is_found_inside_groups_and_the_camera_goes_to_it`.
    Depois: *Find a node* na paleta (`Do::Find`, abre a mesma busca, com o atalho do Ctrl+F) e a câmera deslizando até o
    nó (o glide do F, 3 × `animation_time`; no Problems, `centre_on` em vez do `reveal` instantâneo). Testes:
    `find_a_node_in_the_palette_and_the_camera_glides_to_it`, `a_problems_node_is_glided_to`. Nada em aberto.
13. **Minimapa** no canto do canvas para modelos grandes, com o retângulo da vista arrastável.
    **Feito** (Unreleased, por um sub-agente): `editor_view::Minimap` (o retângulo até 180×120 na proporção do
    escopo, `at`/`back`), registrado com `ui.interact` depois do canvas (cliques e roda não passam); clique centra,
    arrasto paneia, o glide cancelado; `settings.minimap` (× esconde, um botão no canto mostra de novo; também nas
    Settings). Teste: `the_minimap_shows_a_large_model_and_moves_the_view`. Falta: as caixas na cor da categoria
    (cinza hoje); a vista muito fora dos limites cortada; "não cabe" pela câmera de destino durante um glide.
14. **Enquadrar a seleção** (F) além do modelo inteiro (Home).
    **Feito** (Unreleased, pelo mesmo sub-agente do 12): `Camera::fitting_to` (o zoom máximo: 1 no Home, 1,75 no F),
    `EditorState::framing` e `glide` (a câmera desenhada interpolada por 3 × `animation_time`, ease-out cúbico; a
    guardada vai direto; roda ou arrasto cancelam); F na lista do F1. Teste:
    `f_frames_the_nodes_selected_and_home_the_whole_model`. Falta: durante o plano, os cliques usam a câmera de
    destino.
15. **Renomear em lote**: os selecionados com um padrão (`land_{n}`, `{file}`), e o id seguindo o arquivo quando ele
    muda.
    **Feito** (Unreleased, por um sub-agente): `editor::batch_names` (`{n}` na ordem do canvas, `{id}`, `{op}` como
    o `fresh_id`, `{file}` pelo `id_of_file`; os erros por nó), `rename_many` (por ids temporários: trocas e
    cadeias; tudo ou nada), `id_after_file` (só quando o id é o do arquivo antigo); `builder::batch_rename` (a janela
    com a prévia), `renamed` (a seleção e as camadas acompanham), `change_file` renomeando no mesmo passo. Testes:
    `the_selections_rename_follows_a_pattern`, `the_selections_rename_checks_its_names`,
    `the_id_follows_the_file_when_it_changes`. Falta: um id `foo_2` (desempate) não segue o arquivo; o Esc que fecha
    a janela pode chegar ao canvas; um atalho.
16. **Do Viewer ao nó**: no menu de uma camada dos Layers, "Show its node" seleciona e enquadra o nó que a deu
    (abrindo o grupo se preciso).
    **Feito** (Unreleased, por um sub-agente): `find::of_layer` (a camada de um nó aninhado é `node = <container>`
    com `port = "inner.port"`; um id com `/` ou único em qualquer profundidade também), `EditorState::frame` (o F,
    sem o centrar de `find::go`), `Session.ask_model` (o painel Model à frente e a janela principal erguida),
    `Asked.node` em layers_panel.rs. Testes: `a_layers_menu_shows_its_node_on_the_canvas`,
    `a_layer_is_mapped_to_the_path_of_its_node`. Falta: o erguer da janela principal testado (o harness não tem
    janelas do SO).
17. **Paleta e faixa para todas as camadas**: aplicar a paleta (ou a faixa de valores) de uma camada a todas as
    camadas do mesmo tipo de uma vez, para comparar anos lado a lado.
    **Feito** (Unreleased, por um sub-agente): *Same colours for all similar layers* no menu de uma camada;
    `layers_panel::similar` (contínuo com contínuo; classes com os mesmos valores; Packed, Shade, RGB, diferenças
    e vetores fora) e `same_colours` (`s.palettes`/`s.ranges`, os mesmos da escolha à mão, gravados no Plot). Teste:
    `a_layers_colours_are_given_to_all_similar_layers`. Teste de classes para classes: `a_layers_colours_are_given_to_the_layers_of_the_same_classes`. Falta: um passo de
    desfazer (as paletas não têm undo hoje); camadas ainda sem dados não contam.
18. **Fios com redirecionamento** (um ponto no meio do fio, arrastável) e fios que desviam das caixas em modelos
    densos.
    **Feito** (Unreleased, por um sub-agente): `Node.wires` (graph, por porta de entrada, fora do TOML quando vazio;
    os `bare()` do exec e do user_node o tiram); `editor::add_wire_point`/`move_wire_point`/`remove_wire_point`,
    `prune_wires` em toda troca de ligação, `shift_wires` nas cópias e no agrupar; em editor_view.rs o fio como
    cadeia de béziers (`wire_path`), `Drag::Point`, `menu_point`, o desvio (`Scene::detour`: 48 amostras, dois pontos
    acima ou abaixo por caixa, só fios sem pontos, refeito ao mover uma caixa); `settings.wires_around`; o *Arrange*
    (canvas e MCP) limpa os pontos. Testes: cinco em canvas.rs (`a_wire_gets_reroute_points_...`,
    `a_wire_across_a_box_goes_around_it`...) e `reroute_points_are_saved_only_when_there_are_some`. Os testes que mudam ou dependem da `settings` global (minimapa, desvio, fios) seguram um mesmo `settings_lock()` em canvas.rs. Falta: sem dica ao passar sobre um fio (o item 19).
    **Medido com milhares de nós** (Unreleased; `canvas::timings`, release, medianas de 5 rodadas intercaladas, 10.000
    nós, o benchmark liga todos a `n0`, então ~12 mil fios passam pelo nó arrastado): arrastar um nó custava 31 ms por
    quadro (100 nós 0,33 ms; 1.000 nós 2,8 ms), todo em `Scene::follow` (28 ms): a cada passo refazia o desvio de todo fio
    com curva torta (`!bend.is_empty()`) ou com caixa perto, 47 consultas à grade por fio; o resto (`edge_shapes` 1,1 ms,
    `Arc::make_mut` das formas 0,5 ms, o quadro 3 ms) é pequeno. Agora: o nó arrastado fica "levantado" (`Scene::lifted`,
    esvaziado em `settle`), então depois do primeiro passo só os fios ligados a ele são refeitos (o desvio com `skip`
    não depende de onde o nó está); no primeiro, só os fios com um ponto da curva a 4 do lugar velho ou do novo
    (`passes`: bisseção, a curva é monótona em x e y), e `settle` só os ligados e os perto. A grade usa um hasher por
    multiplicação (`CellHasher`). Resultado: arrastar 10.000 nós 32,1 -> 4,7 ms (o pior quadro, o primeiro passo,
    11,6 ms); 1.000 nós 2,77 -> 0,50 ms; 100 nós 0,34 -> 0,16 ms; quadros parados e "edit" (checado 35 ms, desenhado
    81 ms em 10.000) iguais. Teste: `a_node_dragged_leaves_the_wires_as_a_full_look_finds_them` (a cada passo cada fio
    igual ao desvio completo; depois de `settle`, igual a uma cena nova). Falta: os 4,7 ms ainda são O(fios) por
    passo (a varredura dos fios ligados, `edge_shapes` inteiro e o `Arc::make_mut` das formas), um índice de fios por
    nó e só as instâncias mudadas na GPU baixariam mais; o `edit` (35 + 81 ms em 10.000) não foi mexido.
19. **Ver o tipo e o valor numa conexão**: passar o ponteiro sobre um fio mostra o tipo e um resumo do último valor
    (um mapa: tamanho e faixa; um número: ele).
    **Feito** (Unreleased, por um sub-agente): `metadata::brief` (o texto; a faixa até 20 milhões de células),
    `builder::wire_text` (o valor pelo `value_of`, "not run yet" / "changed since the last run"), em editor_view.rs
    `WireAsk`, `WIRE_REST` (0,3 s parado antes do `wire_at`, uma vez por posição) e `EditorState::wire_tooltip`; o
    texto em cache por revisão, execução e fio. Testes: `a_connection_tells_its_type_and_value`,
    `a_connection_of_a_number_shows_the_number`. Falta: dentro de um grupo a frescura é a do grupo inteiro; a faixa varrida
    na thread da UI (uma vez por fio e execução).
    **Feito** (Unreleased): "toda edição marca todos os fios" -- `stale::between` (stale.rs) compara o modelo de
    quando a última execução começou (`Session::ran_model`) com o novo, nó a nó, sem posição, largura, desvios,
    notas e registro; o que mudou, o que lê isso (transitivamente, também dentro dos corpos, por `group/inner`) e o
    que sumiu perde o status; o resto o mantém. Sem esse modelo, limpa tudo como antes.

Fim do bloco do canvas (2026-11-06): itens 4-19 feitos; os itens 1-3 já estavam.

## O Viewer como um SIG: o que ainda falta

Pedido de 2026-11-05: "Gostei de todas as ideias. Pode anotar tudo no IDEAS. Depois pode começar a implementar
todas as ideias." As lacunas do Viewer frente a um SIG e as sobras anotadas nas partes já feitas. Um commit por
item, na ordem; release só quando pedido.

1. **Composição RGB de três bandas.** Uma camada de mapa de uma pilha (ou de um raster de várias bandas) mostrada
   como cor natural ou falsa-cor: a banda do vermelho, do verde e do azul escolhidas na linha da camada, cada uma
   com o seu contraste (percentis 2–98 por padrão, mínimo e máximo à mão, gama). Atalhos para Landsat 8/9 e
   Sentinel-2 (4-3-2, 5-4-3, 8-4-3) quando os nomes das bandas os dizem. Guardada no Plot, na figura e no export.
   **Feito** (Unreleased, por um sub-agente): `rgb.rs`. `Layer.rgb` (`Rgb { bands, ranges, gamma }`), a fonte
   `map_view::Source::Rgb` (os três tiles da banda feitos pelas threads de sempre, as cores empacotadas num f32 de
   24 bits, `Coloring::Packed`), as faixas dos percentis 2–98 por banda numa thread (`rgb::Ranges`,
   `Session::rgb_ranges`). *RGB* nas propriedades da camada: *composites* (`rgb::presets`: cor natural, infravermelho
   e SWIR do Sentinel-2 e do Landsat 8/9 pelos nomes, um sensor só quando o B8A/B12 ou o B10/`SR_` o dizem; senão as
   três primeiras), a banda de cada canal, mínimo e máximo à mão com *auto*, a gama. A legenda de três linhas sob a
   linha, os três valores sob o cursor. No Plot `PlotLayer.rgb` (`PlotRgb`), na figura, no export, na animação, no
   `plot_export` e no `set_plot` do MCP. Testes: os de `rgb.rs`, `a_stack_shown_as_rgb`. Falta: o mapa mostrado sem
   ser camada (é preciso fixá-lo); a comparação (cortina, lado a lado, diferença) usa a banda; sob o cursor, uma
   camada RGB por cima diz só o canal R; o histograma de cada banda.
2. **Selecionar por área e usar a seleção no modelo.** Shift+arrastar no mapa seleciona as feições de uma camada de
   vetores por um retângulo (Shift+Alt: um laço); a seleção guardada no Plot; *Filter the selection* cria um nó
   `vector.filter` com as feições escolhidas; Ctrl+A com o foco do teclado na tabela (não só o ponteiro).
   **Feito** (Unreleased, por um sub-agente): `area.rs` (`Drag`, `Done`, `hits`: o ponto dentro, uma aresta que cruza,
   o polígono que contém a forma), `vector_tiles::Layer::in_shape` (o índice das caixas, depois `hits`),
   `MapView::select_area` antes do pan (Shift+arrastar, Alt um laço, Ctrl soma; Esc solta; nada medindo ou
   desenhando), `Pick::area` (sem levar a vista), na tabela aberta ou na da camada de vetores mais alta.
   `PlotLayer.selected` (os números a partir de 1), de volta no `open_plot`. *Filter the selection*
   (`builder::filter_selection`, `attribute_table::filter_expression`: faixas `a <= feature <= b` ligadas por `or`,
   `not (...)` quando as não escolhidas são menos faixas), um passo de desfazer, o nó fixado por cima do mapa.
   Ctrl+A com o foco do teclado na tabela. Testes: os de `area.rs` e `attribute_table.rs`,
   `the_selection_is_kept_in_the_plot`, `features_selected_by_area_on_the_map`. Também: três testes de UI que
   falhavam às vezes sob carga ficaram firmes (`a_study_area_drawn_on_the_map`, `rename_metadata_and_the_operators_menu`
   repetem o clique; `a_map_kept_as_a_plot_without_writing_and_saved` espera o mapa novo, não o status). Falta: ~~a
   seleção no Plot só da tabela aberta; o `set_plot` do MCP sem `selected`~~ (feitos: `Session.preview_kept` /
   `Shown.kept` por (nó, porta), `Shown::open_table`/`open_with`/`selection_of`, `plot_of` e `open_plot` com todas
   as camadas; `selected` no `set_plot`; testes `the_selection_outlives_the_table`,
   `a_plot_keeps_the_selected_features`; faltam: o mapa destacar a seleção com a tabela fechada, a área medida
   pela seleção guardada, o `kept` limpo quando a camada sai ou o vetor muda, o `selected` das vistas lado a
   lado gravado do Viewer); o laço sem preenchimento (o egui só preenche formas convexas).
3. **A simbologia dos vetores.** Colorir por um atributo com a rampa escolhida (as do Viewer) e as quebras
   (contínua, intervalos iguais, quantis, Jenks; o número de classes), o tamanho dos pontos por um atributo e
   símbolos de ponto além do círculo (quadrado, triângulo, cruz). A legenda dos vetores nos Layers. Guardada no
   Plot, na figura, no export e no projeto do QGIS.
   **Feito** (Unreleased, por um sub-agente): `vector_tiles.rs`. `Style` com `ramp`, `breaks` (`Breaks`: contínua,
   iguais, quantis, Jenks), `classes`, `categories`, `size_by`, `size_min`/`size_max`, `symbol` (`Symbol`), o JSON só
   com o que difere do padrão. `look(v, style) -> Look { colours, sizes, legend }`, a única conta das cores e tamanhos,
   usada pelos tiles (o cache solto com mais de 64 estilos), pela legenda dos Layers, pela figura (`Vectors::of`, a
   legenda dos vetores sob a do mapa), pelo export, pelo `plot_export` e pelo `qgis.rs` (categorizado, graduado, o
   marcador e o tamanho por expressão). `natural_breaks`: Fisher-Jenks ótimo numa amostra de até 1 500 valores
   distintos. O editor em `vector_view::style_controls`. Testes: os de `vector_tiles.rs`,
   `the_legend_of_vectors_colours_and_sizes`, `vector_symbology_in_qgis`, `a_plot_keeps_the_symbology_of_its_vectors`,
   `the_symbology_of_a_vector_layer`. Falta: conferir no QGIS de verdade (`a_project_for_pyqgis`); as quebras numa
   thread (hoje na da janela, uma vez por estilo); a legenda dos tamanhos na cor da classe; numa figura de vários
   painéis, só a legenda dos vetores do primeiro.
4. **O relevo sombreado sob o mapa.** Um modo de mistura por camada (normal, multiplicar, tela) e, numa camada de
   terreno, *Shade with this elevation*: o hillshade feito do próprio mapa (azimute e altura do sol, exagero
   vertical) multiplicado sob as cores. Guardado no Plot, na figura e no export.
   **Feito** (Unreleased, por um sub-agente): `blend.rs` (`Blend`: normal, multiply, screen; `mix` na CPU, em cores
   premultiplicadas; na vista um paint callback do egui_wgpu com pipelines de blend próprios, `blend::install` em
   `canvas_gpu::install`) e `shade.rs` (`Shade`, Horn como o gdaldem, `cell_metres` em graus, `ShadeSource`: cada tile
   de uma janela com uma célula de halo, `Source::halo`). O sombreado é preto com alfa `força*(1-brilho)`
   (`Coloring::Shade`) logo acima da camada: o alfa comum dá `cor*(1-força+força*sombra)`. `PlotLayer.blend` e
   `PlotShade`; `animation::composed`, `plot_export` e o `set_plot` do MCP. Testes: os de `blend.rs` e `shade.rs` (sem
   emenda entre tiles, igual ao gdaldem no interior), `a_map_shaded_with_its_own_relief`,
   `a_layer_blended_with_the_one_under_it`. Falta: ~~camadas reprojetadas (a malha) desenhadas em normal na vista~~
   (feito: `blend::MeshTile`, `vs_mesh`, pipelines de malha com o mesmo `blend_color`, `MapView::blend_mesh`; testes
   `a_mesh_tile_keeps_the_meshs_points_and_places`, `a_reprojected_layer_is_blended_in_the_live_view` com 90% do
   reticulado dentro de 6 por canal, as bordas de classe fora; os buffers da malha refeitos a cada quadro, não
   medido numa janela de verdade); o blend no mapa mostrado; com opacidade abaixo de 1 o sombreado escurece também o que está
   embaixo; num alvo sRGB a GPU mistura em luz linear (o screen difere um pouco do export); a borda do mapa (o
   vizinho que falta conta como a própria célula, o gdaldem `-compute_edges` replica a borda); o blend e o sombreado
   no projeto do QGIS.
5. **Orientar-se no mapa ao vivo.** A barra de escala e, opcional, a grade de coordenadas sobre o mapa (não só na
   *Figure*); voltar e avançar no zoom (Alt+← / Alt+→, botões); *Go to...*: uma coordenada (lat/lon em graus
   decimais ou GMS, ou x/y no CRS do mapa) centrada no mapa com um marcador.
   **Feito** (Unreleased, por um sub-agente): `nav.rs` (`nice_length`, `scale_bar` pelo `measure::cells_apart`,
   `graticule` com `figure::nice_step`, `History`: 50 vistas, guardada depois de 0,4 s parada, o ramo cortado) e
   `goto.rs` (`parse`: com vírgula "lat, lon", sem ela "lon lat" salvo um valor além de ±90, N/S/E/W, GMS; `locate`
   pelo `terraflux_gdal::Transform` de EPSG:4326). `MapView`: `Aids` (das configurações `scale_bar`, `map_grid`),
   `back`/`forward`, `go_to`, a `Mark`; ‹ › e *Go to...* no cabeçalho, *On the map* no painel; F1 e a barra de
   status com Ctrl+G e Alt+←/→. Testes: os de `nav.rs` e `goto.rs`, `the_scale_bar_on_the_live_map`,
   `the_scale_bar_of_a_map_in_degrees_is_in_metres`, `the_scale_bar_without_a_crs_is_in_cells`,
   `back_and_forward_along_the_views`, `go_to_a_coordinate_with_a_mark`. Falta: a grade em lat/lon sobre um mapa
   projetado; ~~a marca nas vistas ao lado; *Go to* com zoom~~ (feitos: `goto::Zoom`, `parse_with_zoom`, o `@`
   no texto e o combo de escalas, `nav::metres_per_point`; `goto::together` e `MapView::paint_mark_beside` pelo
   `Link::between`; testes `go_to_at_a_scale_or_a_width`, `the_go_to_mark_is_in_the_views_side_by_side`; falta a
   marca no comparar com cortina/diferença); ~~o datum de um mapa geográfico que não é WGS 84~~ (feito: *Go to*
   leva os graus digitados, WGS 84, ao datum do mapa pelo PROJ, `goto::degrees_as_is`, teste
   `degrees_reach_a_geographic_map_of_another_datum_shifted` com SAD69; a dica do campo diz "WGS 84"; o inset da
   *Figure* é sempre em WGS 84; a medição segue no elipsoide WGS 84, a diferença para GRS80/SAD69 é menor que 4
   partes em um milhão, só o de Bessel passa de 1 em 10 000: `Geod` com os eixos de `terraflux_gdal::ellipsoid`
   seria o passo se pedirem). ~~A grade em lat/lon sobre um mapa projetado~~ (feita: `nav::curves`, linhas de
   longitude e latitude de 64 pontos pelo PROJ no datum do próprio mapa, `terraflux_gdal::geographic_of`, cortadas à
   vista e rotuladas onde cruzam as bordas; grades rotacionadas também; testes
   `the_grid_of_a_projected_map_is_of_longitudes_and_latitudes`, `the_lines_of_a_projected_map_are_of_its_own_datum`).
   **A investigar:** a suíte `--test ui` caiu duas vezes em doze rodadas com `STATUS_ACCESS_VIOLATION` (o processo
   inteiro, sem dizer o teste), as duas enquanto rodavam `templates_run`, `the_methods_and_citations_of_a_model` e
   `the_model_as_a_python_script`, não os testes novos; sem o item 5, nenhuma em três. Código nativo em paralelo (GDAL,
   landschaft, o GPU): um despejo do Windows (WER LocalDumps) diria onde. 2026-11-14: nenhuma queda em treze
   rodadas (dez com 4 threads de teste, três com as 2 do `.cargo/config.toml`). Candidatos, do mais provável: dois
   devices do wgpu criados e soltos ao mesmo tempo (`create_render_state` em ui.rs ~124 e ~7942, `canvas_gpu::install`;
   confirmar com `WGPU_BACKEND=vulkan` ou um mutex em volta da criação), o `set_var("TERRAFLUX_SETTINGS")` de cada
   teste com outras threads vivas (ui.rs ~108; seguro no Windows, `unsafe` na edição 2024), dois runtimes de OpenMP
   no processo (o vcomp do ls-sys e outro; `dumpbin /dependents`). Quando voltar, rodar com `--nocapture` para ver o
   último teste e ligar o LocalDumps (HKLM, pede administrador).
   2026-11-15: nenhuma queda em ~25 rodadas com `--nocapture` (4 threads); sem evidência, mas duas proteções baratas nos testes:
   o `set_var("TERRAFLUX_SETTINGS")` do `setup()` só uma vez (dentro do `Once`) e a criação do device do wgpu (`render_state()`)
   sob um mutex, um por vez. Se voltar, o último teste do `--nocapture` e o dump do Windows seguem sendo o caminho.
6. **O Viewer numa janela à parte**, para um segundo monitor (`show_viewport_immediate`, como as outras janelas
   soltas), e de volta ao dock.
   **Feito** (Unreleased, por um sub-agente): sobre o `detach`/`attach` que já havia. *Pop out* / *Back into the
   window* no cabeçalho (`Session::viewer_toggle`) e no menu View; `TerrafluxApp::viewer_out` guardado no layout
   (`LAYOUT_VERSION` 12, `read_layout` aceita 11 e 12), a janela "Terraflux · Viewer" com o tamanho e o lugar das
   configurações (`viewer_window`, gravados 600 ms depois de parada; `viewer_window_at` com `virtual_screen` traz de
   volta uma janela fora das telas); `focus` sobe a janela (`ViewportCommand::Focus`); Figure e Export na janela do
   Viewer; o arquivo solto pela viewport do Viewer. Testes: `the_viewer_pops_out_into_a_window_and_back`,
   `the_viewer_out_by_the_view_menu_and_in_by_closing_its_window`, `the_viewer_out_is_kept_between_sessions`,
   `files_dropped_on_the_viewer_in_its_window`. Falta: conferir com o aplicativo de verdade (o kittest desenha as
   viewports dentro da raiz): os callbacks da GPU numa viewport do sistema, o arraste do Explorer na outra janela,
   monitores de DPI diferentes; o Ctrl+W na janela fecha a aba em vez de voltar ao dock.
7. **Dados remotos como camada.** *Add a layer from a URL...*: um COG ou qualquer raster que o GDAL abre por
   `/vsicurl/`, lido por janelas à medida que o mapa pede (as overviews no zoom de longe), e um XYZ/WMTS qualquer
   como basemap (a URL com `{z}/{x}/{y}`).
   **Feito** (Unreleased, por um sub-agente): `terraflux_graph::is_remote`/`remote_path` (e `is_sent`: o endereço
   não é relativizado nem conferido como arquivo), o GDAL configurado para a web uma vez (blocos de 128 KB, cache de
   64 MB, tempos de 10 e 30 s; `GDAL_DISABLE_READDIR_ON_OPEN=EMPTY_DIR` só ao abrir um remoto, por thread), `File::open`
   de um remoto pelo GDAL (o `RasterIO` escolhe a overview). `remote.rs` (*A URL...*: `probe` numa thread),
   `builder::add_url`; o histograma e as estatísticas de um remoto de uma amostra de 1 milhão de células; os
   cabeçalhos de remotos fora da thread da janela. `basemap.rs`: `Basemap::Custom`, `validate`, `register` (5, o
   último primeiro), `custom_window`, o cache por um hash da URL. Testes: `tests/web_layers.rs` (um servidor em
   127.0.0.1 com faixas de bytes: o COG de 22 MB, 404, o servidor que passa a falhar, o GeoJSON, o XYZ próprio, as
   configurações). Também: `every_test_file_runs` aceita os módulos de `ui.rs`. Falta: `{s}` vira `a`; o cache de
   resultados não vê um remoto que mudou; ~~*A URL...* também no cabeçalho do Viewer~~ (feito: o botão no cabeçalho
   abre o mesmo `remote::Dialog`, `remote::URL_HELP` comum; teste `a_url_from_the_viewers_header_opens_the_dialog`).
   **Feito depois** (Unreleased): ~~WMTS~~ (`WMTS:https://.../WMTSCapabilities.xml,layer=...` é um raster:
   `terraflux_graph::service_of`/`is_wmts`, `raster.read`, *A URL...*; também `WFS:`/`OAPIF:` como vetor; teste
   `a_wmts_service_added_as_a_layer_by_its_address` com capacidades e tiles num servidor em 127.0.0.1; o basemap segue
   só XYZ); ~~um vetor remoto lido por filtro espacial~~ (`terraflux_gdal::read_vector_in` com
   `OGR_L_SetSpatialFilterRect`, `vector.read` com `extent`/`extent_crs`; no Viewer `view_vector.rs`: um job por
   (camada, caixa da vista arredondada para fora numa grade grossa), as 6 últimas caixas guardadas, só sobre um mapa já
   mostrado e enquanto o nó não rodou; testes `vector_read_in_an_extent`, `features_are_read_in_a_box`,
   `a_vector_layer_on_the_web_is_read_by_the_view`); ~~credenciais de S3/GS/Azure~~ (`settings::Cloud`, aplicadas
   com `CPLSetConfigOption` global ao iniciar e ao mudar; os segredos no armazém de credenciais do sistema via trait `SecretStore`, com `Memory` nos testes; o resto no arquivo de configurações `0600` no Unix; a CLI e os workers
   usam as variáveis de ambiente; `read_vector` abre pelo mesmo caminho dos rasters). **Falta (dessas):** o basemap
   por WMTS; o Viewer com um vetor remoto sozinho (sem mapa) ainda o lê inteiro, para enquadrar, e a tabela de
   atributos e os estilos por atributo de uma camada lida pela vista só aparecem depois de rodar o nó; ler pela vista
   com o `extent` na saída do `terraflux plot`/figuras; o RGB automático de um WMTS (4 bandas: hoje a banda 1 em cinza
   até escolher *RGB* na camada); ~~testar S3/GS de verdade~~ (testes `#[ignore]` `cloud_public::` em crates/app/tests: um COG público do S3 e um GeoTIFF público do GCS, lidos sem chaves por `raster.read` com `s3://`/`gs://` e pelo https; falta Azure e o teste com chaves de verdade).
   **Visto de passagem:** `surface::the_terrain_is_landschafts` (crates/ops) falha no VRM por 2e-16
   (`0.008092000620180229` contra `...180007`), com ou sem este item: a paridade bit a bit do VRM com o numpy.
8. **As sobras das partes já feitas.**
   - Histograma: as barras em escala logarítmica quando a cor é; o nodata que não é NaN de um raster em disco não
     conta como valor.
   - *Export the view*: a vista comparada (lado a lado, como a figura) e o basemap.
   - `terraflux plot` / `export_plot`: as diferenças (`minus`), a vista comparada, nós dentro de contêineres.
   - Medir: a caixa dos números só na vista medida; clicar numa vista ao lado mede ali, com a sua própria linha.
   - Rótulos: a prioridade pelo tamanho da feição; o projeto do QGIS com os rótulos do estilo.
   **Feito** (Unreleased, por um sub-agente): o histograma em log (`Histogram.log`, as barras iguais no log, a chave
   do cache com a escala); o nodata que não é NaN já ficava de fora (`Rasters::sample` o troca por NaN: a nota estava
   velha; teste `a_raster_on_disk_leaves_out_its_no_data`). *Export the view*: `export_view::Options` (`beside`,
   `basemap`, `opacity`), as vistas lado a lado num PNG (`GUTTER` de 8 px), `basemap::picture` (o basemap na grade
   do mapa, na CPU, por uma malha de 8 px; os tiles do disco ou baixados), `figure::plain_over`. `plot_export`: a
   diferença por `difference::make`, `Made.beside` (as vistas de `compare`), `place_of`/`lookup` para um nó dentro
   de contêiner (também em `tools.rs` e no `cli`). Medir: `measure::together`, cada vista a sua `Measure`, a caixa só
   na última mexida (`Measure.touched`). Rótulos: `labels::rank`/`by_rank`; `qgis::labeling` com o `label_size` do
   estilo. Testes: os de `histogram.rs`, `export_view.rs`, `plot_export.rs`, `labels.rs`, `figure.rs`, `qgis.rs`,
   `the_view_exported_over_a_custom_basemap_fetches_its_tiles`, `measuring_in_the_views_side_by_side_each_with_its_own_line`.
   Também: `a_study_area_drawn_on_the_map` e `a_right_click_traces_a_cell` firmes sob carga (o botão buscado de novo;
   o rastro conferido no estado do app, não no arquivo do último, que os testes dividem). Falta: ~~um Plot com uma
   camada de dentro de contêiner (`g.inner.map`) é recusado pelo `terraflux_graph::check`/`Ref::parse` (o `set_plot`
   do MCP e o `open_plot` a perdem; o `plot_export` só a acha quando o modelo passa na checagem)~~ (feito
   2026-11-14: `terraflux_graph::locate`/`plot_port`, usados pelo check, `place_of`, `open_plot` e `set_plot`); a cortina dividida
   no PNG; o basemap do export é lento num PNG de 8 000 px (não medido); no `plot_export` a vista comparada nas
   cores do mapa principal; com medir ligado, o polígono e o perfil olham só a vista principal.

Fim do bloco (2026-11-06): os oito itens feitos, um commit cada.

## A aplicação: qualidade de vida fora do canvas e do Viewer

Pedido de 2026-11-06: "quais outras melhorias de qualidade de vida para o usuário poderíamos adicionar na interface
gráfica?", depois "gostaria de implementar todas elas [...] Quando terminar de implementar uma ideia, faça commit e
passe para a próxima automaticamente". Os itens 3 e 7 (e o 6) devem funcionar no Windows, no Linux e no macOS.
Conferido antes (grep em `crates/app/src`): não há autosave nem recuperação; fechar a janela, *New model*, *Open...*
e *Open recent* descartam as edições não salvas sem perguntar (só o `open_in_app` do MCP recusa); o
`RequestUserAttention` só é usado pelo `open_in_app`; o `run_progress` dá a fração, sem tempo restante; os tempos
dos nós (`Session::timings`) só aparecem no painel Node e se perdem ao fechar; o `undo::History` guarda os modelos
sem nome de passo nem lista; o aplicativo segura um modelo só (`session: Option<Session>`); o `pt.rs` traduz só o
que o Terraflux escreve (relatório, documento), a interface é toda em inglês.

A ordem: 1, 7, 6, 5, 8, 4, 3, 9, 10, 2 (o 2 depois das abas, para reabrir todas). O 11 (a interface em português)
saiu do plano a pedido (2026-11-06). Cada item: testes (unitários e, quando há interface, do kittest), clippy, fmt, CHANGELOG (Unreleased),
README quando muda o que o usuário faz, a marca **Feito** aqui, um commit local; release só quando pedido. Os itens
3, 6 e 7 também compilados e testados no Linux (WSL); o macOS pelo CI do release.

1. **Não perder edições: perguntar antes, salvar sozinho, recuperar depois de uma queda.**
   - *Perguntar*: `close_requested` da janela principal com o modelo sujo manda `CancelClose` e abre "Save the
     changes to X? Save / Don't save / Cancel"; o mesmo diante de *New model*, *Open...*, *Open recent*, um
     template, o wizard criando um modelo, um arquivo `.toml` solto na janela. Um `TerrafluxApp::guard(Then)` guarda
     a ação pedida e a faz depois da resposta (Save sem arquivo: o diálogo de salvar; cancelado, nada acontece).
   - *Salvar sozinho*: `recovery.rs`. Com o modelo sujo, a cada 30 s depois de uma edição (revisão nova), o `source`
     gravado em TOML numa thread em `data_dir()/recovery/<chave>.toml`, com um `<chave>.json` ao lado (o arquivo do
     modelo ou nenhum, o título, a hora, a versão); a chave é o hash do caminho, ou `untitled-<pid>-<n>`. Os caminhos
     ficam como o `source` os tem (relativos à pasta do modelo, que o `.json` guarda). Enquanto o aplicativo está
     aberto, o `.json` é tocado a cada 30 s (o pulso). Apagados ao salvar, ao escolher *Don't save*, ao fechar normal.
   - *Recuperar*: ao abrir, as cópias cujo pulso parou há mais de 2 min (não são de outra janela do Terraflux aberta)
     viram uma faixa no alto: "Terraflux closed with edits not saved in X (hoje 14:32): Recover / Discard / Later".
     Recover abre o modelo com o seu caminho de antes, sujo (o título com o ponto), um passo de desfazer até o
     arquivo em disco. Funciona igual nos três sistemas (só arquivos e o mtime).
   - Testes: a cópia escrita e lida de volta com o caminho; o pulso; um modelo sujo fechado pergunta (kittest); Don't
     save fecha e apaga a cópia; Recover devolve o modelo sujo.
   **Feito** (Unreleased): `recovery.rs` (`Keeper`: a cópia a cada 30 s quando mudou, o pulso a cada 15 s, as
   deixadas procuradas ao abrir e a cada 30 s, "parada" depois de 60 s sem pulso e nunca do próprio processo; a
   chave leva o pid, então duas janelas com o mesmo modelo não se pisam); `TerrafluxApp::may_leave(Then)`,
   `ask_new`, `ask_open`, `ask_start`, o `egui::Modal` da pergunta e `closing` (o `CancelClose`);
   `Session::worth_keeping` (sujo e com arquivo, ou editado: um template só começado não pergunta; o arranjo do
   template não é passo de desfazer) e `Session::recovered`. Só o aplicativo de verdade liga o `Keeper` (`lib::run`):
   os testes escreveriam na pasta do usuário. Testes: os de `recovery.rs` e `tests/quality_of_life.rs`. Falta: o
   modelo de um `vector.read` ou do wizard ainda não salvo tem caminhos absolutos (ok); a pergunta quando o
   `open_in_app` do MCP chega (hoje recusa, o item 10 abre numa aba).
2. **Reabrir a última sessão.** Configuração "Reopen the models open last" (ligada). Ao fechar (e a cada minuto), em
   `data_dir()/last-session.json`: os modelos abertos (as abas do item 10) e o da frente; de cada modelo, por
   caminho, em `data_dir()/views.json` (os últimos 100): a câmera do canvas (e o grupo aberto), o nó selecionado, o
   que o Viewer mostra (a saída, as camadas, o enquadramento: o mesmo `Plot` que *Update the plot* guarda, sem
   escrevê-lo no modelo) e o painel na frente. Ao abrir um modelo (também pelo *Open recent*) a vista dele volta; o
   que o Viewer mostrava roda pelo caminho quieto (`run_to_show`, como um duplo clique), nunca o modelo inteiro.
   Sem arquivo de modelo na linha de comando e com a configuração ligada, a sessão volta ao abrir. Testes: a vista
   gravada e lida, um modelo reaberto com a câmera e a saída de antes, a configuração desligada.
   **Feito** (Unreleased): `restore.rs` (`Store` numa pasta: `last-session.json` com `Last { models, front }`, os
   arquivos sumidos tirados; `views.json` com uma `View` por modelo, pelo caminho canônico, os últimos 100, escrito
   ao lado e renomeado). `View`: o escopo, a câmera (`EditorState::camera`/`set_camera`), o nó selecionado, o Viewer
   como um `Plot` (`builder::view_plot`: o `plot_of` do *Update the plot* sobre os campos da sessão, com o
   enquadramento do mapa em vista; nunca escrito no modelo) e o painel da frente. `apply`: o escopo, a câmera, o
   Viewer (`builder::show_plot`, separado do `open_plot`), os nós dele na `show_queue` (o caminho quieto, depois de
   ler os dados), a seleção (depois do Viewer, que seleciona o seu nó) e o painel. O `TerrafluxApp::restore` só
   existe no aplicativo de verdade (`lib::run`), como o `Keeper`; `remember` a cada minuto, ao fechar (o
   `may_quit` aceito) e ao fechar uma aba (`drop_front`); `open` devolve a vista; `reopen_last` no `lib::run` quando
   nenhum modelo foi dado e a configuração `reopen_last` (Settings > Start) está ligada. Testes:
   `restore::tests::the_views_and_the_last_session_are_kept` (gravar, ler, o último de cada modelo, os 100, um
   arquivo sumido) e `the_last_session_comes_back_with_each_models_view` (duas abas, a da frente com a câmera, a
   seleção e o Viewer de antes, só o nó mostrado rodado). Ficou de fora: um teste da configuração desligada (ela só
   age no `lib::run`, que os testes não abrem).
   **Feito** (Unreleased), o teste que faltava: o começo saiu do `lib::run` para `TerrafluxApp::start_restoring`
   (a `Store`, se um modelo foi dado, a configuração), e o `the_last_session_comes_back_with_each_models_view` abre
   com um modelo dado (só ele) e com a configuração desligada (um modelo novo) antes de reabrir.
3. **Localizar arquivos ausentes** (Windows, Linux, macOS). `relink.rs`. Ao ler os dados: os caminhos que o modelo
   lê (as entradas `$` com arquivo e as configurações de arquivo dos nós, `terraflux_graph::file_settings`, nos corpos
   dos contêineres também) que não existem. Uma faixa no alto do canvas e no Problems: "12 files not found: Locate
   them...". O diálogo: a lista (o caminho antigo, o achado ou não), "Look in a folder..." e "Locate this one...".
   Achar: os componentes do caminho antigo separados por `/` e por `\` (um modelo do Windows aberto no Linux ou no
   macOS tem `C:\dados\x.tif` como um nome só), a cauda mais longa que existe sob a pasta escolhida; senão o mesmo
   nome procurado na pasta (até 4 níveis, até 20 000 arquivos, numa thread), em minúsculas no Windows e no macOS;
   achado um, a mesma troca de prefixo é tentada em todos (como o Premiere). "Apply" troca todos de uma vez, um passo
   de desfazer, os caminhos relativos à pasta do modelo quando ele tem arquivo, e lê os dados de novo. Também os
   `raster.read` de uma pasta ou de um padrão (a pasta que falta). Testes: a cauda nos dois separadores, a troca de
   prefixo, a busca por nome, o desfazer, um modelo com caminhos do Windows lido no Linux (WSL).
   **Feito** (Unreleased): `relink.rs` (`paths`: as entradas `Map`/`Stack` e os `file_settings` dos nós, nos corpos
   também, menos os de quem escreve, `*.write`, e os enviados ou na web; `missing`, `parts`, `by_end`, `by_name`,
   `locate` com a troca de prefixo, `replace` pelo `relative_path`). `Relink` na `Session`, refeito a cada edição, a
   cada leitura dos dados e a cada 5 s (arquivos copiados nesse meio tempo); a faixa (`strip`) no `builder::bar` e no
   Problems, a janela "Files not found" (um "Choose..." por arquivo, de pasta para um padrão ou um caminho sem
   extensão, e a mesma troca tentada nos outros). O Apply é um passo de desfazer nomeado ("2 files located again") e
   lê os dados de novo. Testes: `relink::tests` (os dois separadores, a cauda, o nome e a troca, o modelo, o padrão),
   `the_files_not_found_are_located_again` (a imagem `ui-relink.png`) e os testes puros compilados no Linux do WSL
   (caminhos do Windows achados lá). Ficou de fora: a busca por nome numa thread (até 20 000 entradas leva menos de
   um segundo; o diálogo de pasta já bloqueia).
   **Feito** (Unreleased), a segunda parte: a busca numa thread (`Search`, com um `AtomicBool` para cancelar;
   `by_name` e `locate_until` param nele), o spinner e o Cancel na janela; ao abrir um modelo salvo, a busca
   automática em volta dele (`around`: a pasta do modelo e a de cima), uma vez por caminho antigo, os achados
   oferecidos na faixa ("2 found beside the model.", *Apply (2)*). `relink_model` e `Relinked::words` para quem não
   tem janela: a ferramenta `locate_files` do MCP (pastas relativas à do modelo) e `terraflux relink` (`--in`,
   `--dry-run`, sai com 1 quando ainda falta algum). Testes: `a_model_is_relinked_around_its_folder`,
   `files_not_found_are_located_by_an_agent`, `the_files_not_found_are_looked_for_around_the_model` e o
   `the_files_not_found_are_located_again` com os dados longe do modelo.
4. **O histórico do desfazer.** O `History` guarda um nome por passo, feito quando o passo fecha (`tick`): o
   `review::diff` do antes e do depois resumido ("raster.calc added: suit", "setting threshold of reclass: 3 → 4",
   "3 nodes moved", "2 nodes deleted", "connected a.map → b.x"), ou o nome dado por quem editou (o Assistant, o
   MCP, um rename em lote). Um painel History (View, ctrl+K, e o menu do botão de desfazer): os passos, o atual
   marcado, os desfeitos em cinza; um clique vai até ele (desfazer ou refazer vários de uma vez, uma ida só);
   "Compare with now" abre o Changes com aquele modelo (`review::compare`). Testes: os nomes dos passos, ir e voltar
   pelo painel.
   **Feito** (Unreleased): `undo.rs` guarda os nomes ao lado dos modelos (`undo_names`, `redo_names`), feitos no
   `tick` que fecha o passo (`describe`: o `review::diff` resumido; sem diferença de nós, o que moveu) ou dados por
   quem editou (`History::name`: o Assistant com o nome da ferramenta, o rename em lote); `steps`, `model_at`,
   `go_to` (vários `undo_redo` de uma vez) e a janela `undo::window` (uma janela, não um painel: abre por cima do
   canvas; `Session::history_open`), com "compare". Testes: `undo::tests` e
   `the_history_goes_back_and_forward_to_any_step` (a imagem `ui-history.png`). Falta: o MCP nomear os seus passos
   (as edições dele chegam pelo arquivo, `watch::take`: hoje "added …" pelo diff).
   **Feito** (Unreleased), o que faltava: `agent_edits.rs` (`tell`: o `mcp::change` conta a ferramenta antes de
   escrever o arquivo, em `agent-edits.json` nos dados do aplicativo, pelo caminho canônico, com a hora; `take`:
   o `watch::take` as pega e esquece, as de mais de dez minutos de fora; `step_name`: "the MCP: add_node, connect",
   cada ferramenta uma vez, mais de três "X and N more"), o nome dado ao passo com `History::name`. Testes:
   `agent_edits::tests::the_tools_told_name_the_step` e `the_steps_of_an_agent_are_named_by_its_tools` (ui).
5. **O tempo que falta.** `eta.rs`: com os tempos de cada nó das execuções anteriores (`Session::timings`, agora
   também gravados por modelo em `data_dir()/run-times.json`, os últimos 200 modelos), o que falta é a soma dos nós
   ainda não feitos mais o resto do que roda (pela sua fração: um laço pelos passos, um operador pelo que ele
   diz); sem tempos antigos, pela fração e o tempo passado (só depois de 5 s e de 3 % feitos, para não chutar). Na
   pílula do progresso: "~14 min left", e no tooltip a hora prevista do fim; "about" quando a conta é pela fração.
   Os nós do cache não contam. Testes: a conta com e sem tempos antigos, um laço pela metade, nós do cache.
   **Feito** (Unreleased): `eta.rs` (`left`, `words`, `Kept`), `Session::time_left` (os nós de cima ainda não
   feitos pelos tempos desta sessão, senão os de antes: `times_before`, lidos ao abrir o modelo e gravados ao fim de
   cada execução; o que roda pela fração que diz, desde `node_began`), `run_going` dá a fração do nó que roda. Testes:
   `eta::tests`, `the_time_left_comes_from_the_runs_before`. Falta: a hora prevista do fim (sem um relógio local no
   std; o `chrono` não está no app); os nós que virão do cache contam o tempo de antes (não se sabe antes de rodar).
   **Feito** (Unreleased), a hora do fim: `eta::ends_at` ("ends about 14:32", "... tomorrow", "... in 3 days") no
   tooltip da barra, junto com o "kept awake". A hora local sem crate novo: `localtime_r` do `libc` no Linux e no
   macOS, `FileTimeToSystemTime` + `SystemTimeToTzSpecificLocalTime` do `windows-sys` no Windows (a feature
   `Win32_System_Time`); os dias pelo `days_from_civil`. Teste: `eta::tests::the_end_is_told_by_the_clock`; o
   código do Unix rodado no Linux do WSL e o do Windows aqui, os dois com a hora certa.
6. **Aviso ao fim de uma execução longa** (Windows, Linux, macOS). Quando uma execução pedida (não as quietas) ou um
   lote do Many runs termina depois de mais de 10 s (configuração "Tell when a run ends": ligada, e os segundos) e a
   janela não tem o foco: `RequestUserAttention` (pisca na barra no Windows, salta no Dock do macOS, a dica de
   urgência no Linux) e uma notificação do sistema ("model.toml: done in 12 min", ou a falha e o nó). A notificação
   pelo `notify-rust` (toast no Windows, Central de Notificações no macOS, D-Bus no Linux) se as dependências forem
   leves (conferir o `cargo tree`; o crate `windows` já está no Cargo.lock); senão por processos: `osascript` no
   macOS, `notify-send` no Linux, o toast do PowerShell no Windows. Enquanto roda, o título da janela começa com a
   porcentagem ("[42%] modelo — Terraflux"), visível na barra de tarefas. Testes: quando avisa (tempo, quieta,
   foco), o texto; a notificação à mão nos três sistemas.
   **Feito** (Unreleased): `notify.rs` por processos (o `notify-rust` pede Rust 1.89 e traria o zbus e outro crate
   `windows`): no Windows o PowerShell (`-Command -`, o script pela entrada, o título e o texto pelo ambiente, sem
   janela) registra `HKCU\Software\Classes\AppUserModelId\Terraflux.Terraflux` (nome e ícone, o logo gravado na
   pasta de dados) e mostra o toast; conferido aqui: o toast na Central de Ações sob "Terraflux"
   (`ToastNotificationManager.History`). macOS `osascript` (os textos como `argv`), Linux `notify-send` e depois
   `gdbus`. `Session::ended` (`Ended`: segundos, a falha, se quieta), `TerrafluxApp::tell` (o
   `RequestUserAttention` e a notificação; `told` guarda o que foi dito, e só o aplicativo de verdade manda ao
   sistema), `in_front` pelo `InputState::focused` (a janela principal e a do Viewer). O fim de um lote do Many
   runs também. Configurações `notify_runs` e `notify_after` (Settings > Runs). Testes: `notify::tests`,
   `the_end_of_a_run_is_told_when_terraflux_is_not_in_front`; `notify::by_hand` (ignorado) mostra uma de verdade.
   Falta: um clique na notificação trazer o Terraflux para a frente (pede um ativador COM no Windows); conferir no
   macOS e num Linux com área de trabalho (o WSL não tem servidor de notificações).
   **Feito** (Unreleased), o clique, sem ativador COM: no Windows o toast com `activationType='protocol'` e
   `launch='terraflux-front:<processo>'`, o protocolo registrado pelo próprio script do toast em
   `HKCU\Software\Classes\terraflux-front` (o `terraflux.exe` que roda, `front "%1"`; só quando o programa é o
   `terraflux`, não o de um teste); `terraflux front` (escondido) pede à aplicação daquele processo, pelo `handoff`
   (`Request::path` agora `Option`: nenhum, só vir para a frente), que venha para a frente. No Linux,
   `notify-send --action=default=Open --wait` primeiro (diz "default" na saída quando clicado: `Minimized(false)` e
   `Focus` mandados de lá), depois o `notify-send` de antes e o `gdbus`. O script do toast tem uma instrução por
   linha: lido da entrada, o PowerShell só roda um bloco de várias linhas depois de uma linha vazia (o primeiro
   teste de verdade não registrou nada por isso). Testes: `notify::tests::the_ways_of_each_system` (o protocolo, o
   `--wait`, o `front_of`), o `handoff` com o pedido de vir para a frente e `notify::by_hand::a_click_on_a_toast_
   reaches_terraflux` (ignorado; rodado aqui com o `terraflux.exe` de debug: o toast, o protocolo aberto como o
   clique abre, o pedido chegando). O `notify-send` 0.8.3 do Ubuntu 24.04 tem `--action` e `--wait`. Ficou de
   fora: o macOS (o `osascript` não diz o clique; pediria um app empacotado com `UNUserNotificationCenter`).
7. **Não deixar o computador dormir durante uma execução** (Windows, Linux, macOS). `awake.rs`: um guarda segurado
   enquanto uma execução, um lote ou o empréstimo deste computador a outro trabalha. Windows:
   `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)` (windows-sys, `Win32_System_Power`), pedido e
   devolvido numa thread própria que vive enquanto o guarda; macOS: `caffeinate -i -w <pid>` como processo filho,
   morto ao soltar; Linux: `systemd-inhibit --what=idle:sleep --who=Terraflux --why=... --mode=block sleep
   infinity`, morto ao soltar (sem `systemd-inhibit`: nada, e as configurações dizem). A tela ainda pode apagar.
   Configuração "Keep the computer awake while a model runs" (ligada); o tooltip do progresso diz "keeping the
   computer awake". Testes: o guarda pega e solta (no Windows a chamada; no Linux o processo, no WSL), a linha de
   comando de cada sistema, nada quando desligado.
   **Feito** (Unreleased): `awake.rs` (`Awake::want` a cada quadro; `TerrafluxApp::awake`, só no aplicativo de
   verdade); o `systemd-inhibit` segura `tail --pid=<terraflux> -f /dev/null`, então uma queda solta a trava; no
   Linux tenta `idle:sleep` e, recusado (o WSL não tem sessão do logind: "Access denied"), `idle` só, sem travar a
   interface (o processo que termina logo é visto no quadro seguinte). Conferido no WSL: `systemd-inhibit --list`
   mostra "Terraflux idle block" enquanto segura e nada depois. Configuração `keep_awake` (Settings > Runs) e o erro
   dito ali. Testes: `awake::tests`, `the_computer_is_kept_awake_while_a_model_runs`. Falta: conferir no macOS (o
   CI do release compila; o `caffeinate -i -w` é o de sempre).
8. **O tempo de cada nó no canvas.** Depois de uma execução, cada caixa diz no canto de baixo quanto levou ("2.4 s",
   "3 min"; um contêiner a soma do corpo; cinza quando veio do cache) e o tooltip a memória do que deu. Um botão
   "Times" na barra do canvas pinta as caixas pela parte do tempo total (as mais lentas na cor quente, as rápidas
   sem cor) e lista no canto os 5 nós mais lentos, cada um levando a câmera até ele. Testes: o texto do tempo, a
   escala de cores, a lista dos mais lentos.
   **Feito** (Unreleased): `EditorState::times` (os do escopo, dados pelo `builder::canvas` a cada quadro a partir
   de `Session::timings`; no alto, os nós do cache como "cache") e `times_shown`; o tempo sob o canto direito da
   caixa (fora dela: não cobre o arquivo) com o tooltip; `heat`, `slowest_of` e a lista (`egui::Area` no canto,
   um clique seleciona e centra); `eta::took`. Teste: `the_canvas_tells_how_long_each_node_took` (a imagem
   `ui-times.png`). Falta: os tempos antes de rodar nesta sessão (os de `times_before` poderiam aparecer em cinza).
   **Feito** (Unreleased), o que faltava: `EditorState::times_before` (os do escopo mostrado que não rodaram nesta
   sessão, dados pelo `builder::canvas` a partir de `Session::times_before`), escritos mais apagados e com `~`
   ("~2.4 s"; "<0.1 s" sem), com o seu tooltip. Teste: o fim de `the_time_left_comes_from_the_runs_before` (a
   imagem `ui-times-before.png`), que agora usa um caminho só da sua execução (o `run-times.json` é de todos os
   testes, e apagá-lo corria com o outro teste de tempos).
9. **Notas e molduras no canvas.** No modelo (`terraflux_graph`): `[[notes]]`, cada uma com `id`, o `scope` (o
   contêiner onde está), o texto, a cor, e o lugar (uma nota solta) ou os nós que emoldura (uma moldura: o retângulo
   é o dos seus nós mais uma margem, então segue o *Arrange* e os arrastes). Fora do grafo: o `check` e a execução
   não as veem. No canvas: botão direito no vazio, *Add a note here*; no menu da seleção, *Frame the nodes
   selected*; a moldura desenhada atrás das caixas com o título no alto; arrastar o título arrasta os nós dela; um nó
   solto dentro dela entra, arrastado para fora sai (com alt); duplo clique edita o texto; Delete apaga; a cor no
   menu. Desfazer, copiar e colar, o documento (as molduras como subtítulos dos passos) e o MCP (`set_note`,
   `remove_note`; o `show_model` as lista). Testes: o modelo gravado e lido, a moldura seguindo os nós, arrastar o
   título, entrar e sair, o desfazer.
   **Feito** (Unreleased): `terraflux_graph::Note` (`id`, `scope`, `text`, `colour` por nome, `at` ou `nodes`) em
   `Model::notes`, com `notes_of`, `new_note_id`, `notes_renamed` (chamado pelo `editor::rename_node`) e
   `tidy_notes` (no `Session::edited`: os nós apagados ou agrupados saem das molduras, a vazia some). `notes.rs`:
   `layout` (a moldura é o retângulo dos nós na cena mais a margem e o título, então segue os arrastes ao vivo),
   `hit` (a nota inteira, a moldura só pelo título), `frame_at`, `draw` (antes do callback da GPU: atrás das
   caixas), o menu e a janela do texto. No `editor_view`: o `Drag::Note`; o título de uma moldura começa um
   `Drag::Move` dos seus nós (o mesmo caminho do arraste de nós); soltar nós fora de molduras dentro de uma:
   `JoinFrame`; clique seleciona (`note_chosen`), Delete apaga (no `builder`, antes dos nós), duplo clique edita,
   botão direito no vazio *Add a note here*, no menu da seleção *Frame them*, no de um nó *Take it out of the
   frame*. As ações das notas (`builder::note_action`) não são uma edição do grafo: `history.changed` e um nome
   ("added a frame", "a note moved"), sem apagar o estado dos nós. O Document: as molduras do nível de cima viram
   seções do seu estágio com o título e o resto do texto como nota (`Section::frame`). O MCP: `set_note` (cria ou
   muda; sem lugar, acima dos nós à esquerda) e `remove_note`; o guia do MCP fala delas. Testes:
   `the_notes_and_frames_are_kept_with_the_model` (graph), `notes::tests`, `notes_and_frames_are_set_and_removed`
   (tools), `a_frame_of_the_canvas_is_a_section_under_its_title` (document) e o do canvas
   `notes_and_frames_are_put_on_the_canvas_and_follow_their_nodes` (a imagem `ui-notes.png`). Mudou do plano: sair
   da moldura é pelo menu do nó, não por arrastar com alt (o alt já tira o nó das conexões). Ficou de fora: copiar e
   colar levar as notas e molduras junto com os nós.
   **Feito** (Unreleased), o que ficou de fora: o texto copiado leva `terraflux_notes` ao lado dos nós
   (`editor::clip_text`/`clip_nodes`); vão as molduras com todos os seus nós copiados (`frames_around`), ou a nota
   ou a moldura selecionada (a moldura com os seus nós; `builder::chosen_with_notes`); `paste_notes` dá ids novos,
   a moldura em volta das cópias (sem nenhum dos seus nós colado, fica de fora), a nota com o mesmo deslocamento.
   Vale para ctrl+C/ctrl+V, ctrl+D, o Duplicate do menu e o arraste com ctrl. Testes:
   `notes_and_frames_are_pasted_with_their_nodes` (editor) e `notes_and_frames_are_copied_with_their_nodes` (ui).
10. **Vários modelos abertos em abas.** `TerrafluxApp` com o modelo da frente em `session` (o resto do código não
    muda) e os outros em `others: Vec<Session>`; trocar de aba troca os dois. Uma faixa de abas sob a barra quando há
    dois ou mais: o nome, o ponto de não salvo, um girador quando roda, o x (e o botão do meio) que fecha (pergunta
    pelo item 1). ctrl+Tab e ctrl+shift+Tab passam, ctrl+W fecha. Abrir, um template, o wizard e o `open_in_app`
    abrem numa aba nova (o modelo vazio sem edições é trocado); um modelo já aberto vai para a aba dele. As outras
    abas continuam rodando (`poll` e `auto_run` de todas a cada quadro; o aviso do item 6 diz qual). Os painéis que
    guardam algo de um modelo (Trace, Parameters, Calculator, a revisão, a conversa do Assistant) seguem a aba da
    frente. Copiar nós numa aba e colar em outra. Testes: abrir dois, trocar, rodar um atrás, fechar perguntando,
    colar de um no outro.
    **Feito** (Unreleased): `tabs.rs`. `TerrafluxApp::others: Vec<Parked>` (a `Session` com os estados do Trace, do
    Parameters, do Calculator e do `Chat`, trocados com os do aplicativo ao trocar de aba) e `front`, o lugar da da
    frente entre as abas. `put_in_front` (o `open`, o `start_model`, o `new_model` e o `recover` passam por ele: aba
    nova depois da da frente, ou troca o modelo vazio sem nada feito), `tab_of` (o `open` de um modelo aberto vai à
    aba dele), `switch_to`, `next_tab`, `close_tab` (pergunta pelo item 1 com o novo `Then::Close`; `drop_front` traz
    a vizinha), `may_quit` (cada modelo com edições é trazido à frente e perguntado, um depois do outro: o
    `Then::Quit` confere de novo). Os `ask_new`, `ask_open` e `ask_start` não perguntam mais (nada é posto de lado).
    A cada quadro, `poll_behind`: `poll`, `watch::follow`, `auto_run`, o `Chat`, a revisão das de trás, e o aviso do
    item 6 com o nome do modelo; o `Keeper` do item 1 copia todos; o `awake` vale para todos. O `open_in_app`
    (`handle_handoff`) abre numa aba e não recusa mais um modelo com edições (só com o wizard aberto). A faixa
    (`tabs_strip`): nome, ponto, girador, ×, botão do meio; ctrl+Tab, ctrl+shift+Tab, ctrl+shift+W; na paleta e no
    F1. Copiar e colar entre abas já funcionava (a área de transferência do sistema). Testes:
    `models_open_in_tabs_run_behind_and_close` (abrir dois, a faixa, trocar por ela e por ctrl+Tab, abrir de novo vai
    à aba, os estados dos painéis por aba, uma execução atrás até o fim, o `open_in_app`, fechar pelo ×; a imagem
    `ui-tabs.png`) e os do item 1 refeitos (fechar a aba é que pergunta). Mudou do plano: fechar a aba é
    ctrl+shift+W, porque o ctrl+W já fecha o painel da frente.

## A aplicação: execuções, versões, disco, atalhos e lugares

Pedido de 2026-11-06: "existe outras adições e modificações que poderíamos fazer para melhorar ainda mais a qualidade
de vida do usuário?", e das ideias dadas, "gostei de todas [...] pode adicionar ao IDEAS.md" (as de número 1 a 4, 7 e
8 da conversa; a página inicial e a pasta vigiada ficaram de fora). Conferido antes (grep em `crates/app/src`,
`crates/exec`, `crates/ops`): nada guarda as execuções passadas (o `Session::timings` guarda só os tempos dos nós);
não há execução de conferência numa parte do mapa nem em resolução menor (o `raster.read` não tem janela nem
extensão); as versões de um modelo só existem pelo git (o `review.rs` compara com o arquivo, outro arquivo ou o
último commit); nada mede o espaço livre do disco nem estima o tamanho das saídas; os atalhos estão fixos no código
(86 `consume_key` em `crates/app/src`, listados no F1); não há lugares marcados no Viewer nem no canvas.

1. **O diário das execuções (painel Runs).** Cada execução pedida (não as quietas) fica registrada ao lado do modelo:
   quando começou, quanto levou, como terminou (fim, parada, erro), o cenário e os valores dos parâmetros, uma marca
   da versão do modelo (o hash do modelo expandido, e o próprio modelo guardado quando muda), as saídas escritas
   (caminho, tamanho, hash) e os avisos. Um painel lista as execuções; uma escolhida reabre as suas saídas no
   Viewer, duas escolhidas se comparam (os parâmetros lado a lado, as diferenças do modelo pelo `review.rs`, os
   mapas pela comparação do Viewer, `difference.rs`), e *Restore this model* volta o modelo ao daquela execução (um
   passo de desfazer). Um lote do "Many runs" é uma entrada só, com as suas rodadas dentro. O `terraflux run` também
   escreve no diário; o MCP lê dele. Em aberto: onde guardar (uma pasta `.terraflux/runs` ao lado do modelo, ou nos
   dados do usuário) e quanto guardar (as saídas não são copiadas, só registradas; uma saída sobrescrita depois é
   marcada como não mais aquela).
   **Feito em parte** (Unreleased): o diário e o painel Runs. Novo `terraflux_exec::journal` (`Journal::of`, `record`,
   `entries`, `model`, `size`; `Entry`, `Status`, `Kind` (run, to, batch), `Batch`, `Factor`, `Written` com
   `unchanged` (tamanho e hora em milissegundos), `record_run`, `values_of`): guardado em
   `<pasta do modelo>/.terraflux/<arquivo>/` (decidido: ao lado do modelo, não nos dados do usuário), `runs.jsonl` (uma
   entrada por linha, com `#[serde(default)]` em tudo, uma linha que não lê é pulada) e `models/<sha256>.json` (o
   `source` do modelo como rodou, uma vez por hash; o `restore` do próximo passo lê dali). As saídas só são
   registradas (a hash e o tamanho vêm do manifesto); uma saída escrita por cima é marcada "overwritten since".
   Aplicativo: `Session::journal_run`/`journal_ended` (a rodada pedida, inteira ou até um nó; `quiet_run` nunca; o
   manifesto de `Session::recorded` dá as saídas) e `BatchState::journal` (um lote ao acabar, parado ou com erro; os
   fatores da sensibilidade ordenados por `batch_view::ranked_factors`). Painel `Pane::Runs` (`runs_view.rs`: `State`
   relê o arquivo quando a sessão ou um lote escreve, ou o tamanho muda, e olha as saídas a cada 2 s; `changes`,
   `kind_text`, seleção de uma ou duas por clique e ctrl+clique em `State::selected`, só o botão *Open folder*) e
   `eta::stamp`. CLI: `terraflux run` (também a falha da execução; `--no-journal`), `terraflux sensitivity` e o MCP
   (`run_model`, com ou sem erro, e `sensitivity`, com `from` cli ou mcp). Testes: cinco em `exec::journal::tests`,
   quatro em `runs_view::tests`, `the_runs_are_kept_and_listed` e o lote em `the_model_runs_many_times_from_its_pane`
   (ui.rs; a imagem `ui-runs-1.png`), `a_run_is_recorded_in_the_journal_unless_asked_not_to` e as asserções do diário
   em dois testes do MCP. Ficou de fora: o cenário aplicado no aplicativo (o app não guarda o nome do cenário
   aplicado; o campo `scenario` só vem do CLI e do MCP, o resto está em `values`), o diário do `submit` e do
   `calibrate`, a falha do `terraflux run` fora da execução (leitura das entradas, `--set` errado) e `--tiled`, e as
   rodadas retomadas depois de fechar o aplicativo. As versões com nome são o item 3 (feito).
   **Feito** (Unreleased), as ações do painel: abaixo da tabela, a execução escolhida em detalhe (todos os valores, as
   saídas com caminho, tamanho e a marca "as written", "overwritten since" ou "missing", os avisos, a mensagem da
   falha, o manifesto, a versão e de onde rodou; um lote com a pasta e, na sensibilidade, os fatores ordenados com o
   índice principal e *Open the folder*). Uma escolhida: *Show its outputs* (`runs_view::show_outputs`: as saídas
   ainda como escritas entram como camadas do Viewer por `builder::drop_on_viewer`, isto é, um nó de leitura cada,
   um passo de desfazer; as sobrescritas, as que sumiram e as que não são mapa, feições nem tabela ficam de fora e
   são contadas no aviso e ao lado do botão) e *Restore this model* (`runs_view::restore`: o `Journal::model` no
   lugar do `source`, uma edição com o passo "Restore the model of the run of <hora>"; desligado, com o motivo,
   quando o instantâneo sumiu ou já é o modelo aberto). Duas escolhidas: os valores lado a lado (`value_rows`: só os
   que diferem, uma caixa para todos), *Review the model's differences* (novo `review::compare_two`: o `Review` ganha
   `after`, e com ele a revisão é só de leitura, sem aceitar nem rejeitar, sem marcas no canvas; `begin` a descarta),
   *Newer against the model open* (o `review::compare` de antes, com aceitar e rejeitar) e *Compare the maps* / *Their
   difference* (um mapa que as duas escreveram, com o mesmo nome, em arquivos diferentes e ainda como escritos:
   `builder::read_to_show` lê os dois e `Session::compare_asked` põe o `preview_compare` quando os dois rodaram, lado
   a lado ou em diferença). O cenário: `Session::scenario_applied` (o painel Parameters guarda o nome e os valores ao
   aplicar) e `Session::scenario_now` (vale enquanto os valores que ele pôs continuam no modelo); `journal_ended`
   grava em `scenario`. MCP: a ferramenta `list_runs` (`tools::list_runs`; no `terraflux mcp` e no Assistente do
   aplicativo; `limit`, `id`; só leitura), um item no guia `results` e uma linha na tabela de
   `website/content/cli/assistants.md`. Testes: `the_runs_are_shown_restored_and_compared` (ui.rs; as imagens
   `ui-runs-detail.png`, `-compare.png`, `-review.png` e `-maps.png`), três em `runs_view::tests`, um em
   `tools::sensitivity_tests` e as asserções de `list_runs` no teste do MCP. Ficou de fora: "Open in Many runs" para
   o lote (só *Open the folder*; reabrir um lote ali pede uma leitura dos resultados que não existe como entrada
   simples), a comparação dos mapas só para saídas raster em arquivos distintos (a execução que sobrescreve os
   arquivos da anterior não tem o que comparar), e a revisão de duas versões passadas sem os mapas antes e depois.
2. **Rodar como conferência numa parte pequena.** *Run on the view* (só a extensão que o Viewer mostra) e *Run at ¼
   resolution* (ou ½, ⅛): o modelo roda em segundos para mostrar se está certo antes de uma execução de horas. Pede
   ao `exec` uma janela e um fator de reamostragem que todo `raster.read` aplica ao ler (lendo só a janela do
   arquivo, as overviews do GDAL quando houver), e os rasters gerados (`nlm.*`) na mesma grade. É marcada como
   conferência: as saídas vão para uma pasta temporária, não sobrescrevem as de verdade, e o Viewer, o canvas e o
   diário dizem "check run, ¼ resolution". O aviso que vem junto: métricas de paisagem, distâncias e vizinhanças
   mudam com a resolução e a borda, então o resultado confere o modelo, não o substitui. O mesmo no `terraflux run`
   (`--window`, `--coarsen`).
   **Feito** (Unreleased): novo `terraflux_raster::part::Part` (a janela de uma grade levada para fora até as bordas
   das células, `of`; um a cada N, o do meio de cada bloco, então classes seguem classes, `take`/`source_col`; a grade
   nova, `grid`) e `terraflux_ops::check::Check` (`window` + `crs` + `coarsen` + `out`; `part_of` leva o bbox para o CRS
   do arquivo com o GDAL e recusa cedo, dizendo o raster, quando ele não tem CRS e a janela tem, quando a grade é
   girada ou quando não há células em comum). Canal escolhido: o `Registry` guarda a conferência (`Registry::checked`,
   `Engine::checked`; nenhum literal `Engine { registry }` dos testes mudou) e o `Engine::run_op` a entrega ao `Ctx`
   (`ctx.check()`) e reescreve os settings do nó (`Check::params`). Cobertura: `raster.read` e as entradas de mapas
   (`load_inputs_checked`) pela nova variante `files::File::View` (a janela e o passo sobre o `File` de verdade: o
   `Rasters` continua preguiçoso, `read_window` lê só os blocos que toca, em faixas de 16 linhas; `sample` do Viewer
   também), os `data.*` que baixam rasters, os `nlm.*` com `rows`/`columns` (linhas e colunas divididas, `resolution`
   multiplicada; com `like` fica a grade dele), e as ferramentas que trabalham no arquivo pelo caminho (GDAL, os
   tiled: porta `Raster`) recebem o recorte como arquivo próprio no cache (`Rasters::materialized`, em `as_port`).
   Os arquivos que o modelo escreve (`raster.write`, `table.write`, `vector.write` e os `prepare` com `path`) vão para
   a pasta da conferência pelo nome do arquivo, nunca por cima dos de verdade. Nós congelados rodam (o cache da
   conferência é novo: sem `hold`, sem `freeze_new`, e `Session::freeze` recusa com uma conferência na tela). Cache:
   decidido por um cache separado, não por limpar; `Session::start_run` guarda o normal em `real_cache` e põe um novo,
   a próxima execução normal devolve o normal (os resultados de horas continuam lá) e limpa a marca. Pasta: `<pasta
   do modelo>/<modelo> runs/check` (`journal::check_dir`), apagada e refeita a cada conferência, com o manifesto.
   Aplicativo: o `▾` ao lado do Run e a paleta (`Do::Check`): na vista (a extensão de `preview_view.visible()` na
   CRS do mapa mostrado, `Session::view_window`; desligado sem mapa), a 1/2, 1/4, 1/8 e na vista a 1/4; a marca
   laranja "check run, 1/4 resolution" no Viewer, na barra de estado e no título da janela, com o aviso no hover
   (`runs_view::check_mark`); o diário grava `Kind::Check(Check)` ("Check run (1/4)", "(view)", "(view, 1/4)" no
   painel Runs), o *Clean outputs...* lista os arquivos como "(check run)" e nunca como os da execução mais nova, e a
   verificação do disco não passa por aqui (`start_check` não chama `disk_ok`; `from_journal` só olha `Kind::Run`).
   CLI: `terraflux run --coarsen N --window xmin,ymin,xmax,ymax [--window-crs]` (a janela no CRS do primeiro raster do
   modelo, o das entradas de mapa e depois os `raster.read`; sem CRS, nas coordenadas de cada arquivo), saídas onde o
   `-o` diz (sem ele, a pasta da conferência, nunca `output`), sem os congelados, `--tiled` recusado, diário `Check`.
   Testes: três em `raster::part::tests`, quatro em `ops::check::tests`, cinco em `exec/tests/check.rs` (coarsen,
   janela, janela fora, escrita redirecionada, entradas), um em `clean_view::tests`, um no `runs_view::tests`, um no
   `cli::tests` (`check_of`) e `a_check_run_gives_the_model_at_a_coarser_resolution_apart_from_the_real_outputs`
   (ui.rs; as imagens `ui-check-menu.png` e `ui-check-marked.png`). Ficou de fora: os operadores que abrem um raster
   pelo caminho nos próprios settings (os `.ini` do CIRCUIT, a pasta do SWAT, `env90m`, os `sdm` de referência) e os
   que chamam outros operadores por dentro (`batch`, `futures_tools`: `Ctx::default()`), `nlm.curds` (o tamanho vem
   dos passos), a janela nos `nlm.*` (não têm georreferência: só o `coarsen` os muda), as overviews do GDAL (o
   `View` lê o nível cheio e escolhe as células), a conferência a partir do MCP e do Assistente, o `--tiled`, o
   `terraflux run` pelo binário de debug no Windows (estoura a pilha até em `--version`: o caminho foi conferido pelos
   testes de `check_of` e do motor, não pelo binário) e uma conferência "na vista" depois de outra (a vista é a do
   mapa mostrado, que pode ser o da conferência: a janela vale nas mesmas coordenadas, e vale).
3. **Versões do modelo com nome, sem git.** *Save a version...* com um rótulo ("antes de trocar o cenário") guarda
   o modelo como está; uma lista das versões (rótulo, hora, o resumo das mudanças) abre qualquer uma em revisão
   contra a atual pelo `review.rs` (aceitar ou rejeitar cada mudança, os mapas antes e depois) ou a restaura (um
   passo de desfazer). Guardadas junto do diário (item 1), que já guarda o modelo de cada execução: uma execução
   também é uma versão, sem rótulo. No MCP, `save_version` e `list_versions`, para o agente marcar um ponto antes
   de uma mudança grande.
   **Feito** (Unreleased): `terraflux_exec::journal` ganha `Version` (`id`, `when`, `label`, `model`, `note`, `from`,
   tudo com `#[serde(default)]` menos o id), `Journal::save_version` (o instantâneo no mesmo `models/`, uma vez por
   hash; um modelo igual ao da versão mais nova é guardado de novo, a versão é um ponto no tempo), `versions` (a mais
   antiga primeiro), `versions_size` e `delete_version` (reescreve `versions.jsonl`; o instantâneo fica se uma
   execução ou outra versão o usa). Aplicativo: novo `versions_view.rs`: *Save a version...* no menu File, na paleta e
   no F1 (ctrl+alt+S, lida antes do ctrl+S, que pega a tecla com alt também), uma janela com o rótulo (obrigatório) e
   a nota, que avisa quando o modelo tem edições não salvas (a versão é do que está na tela) e pede para salvar o
   modelo antes quando não tem arquivo. O painel Runs ganha as abas **Runs** e **Versions** (`runs_view::Tab`): as
   versões da mais nova para a mais antiga, com a data, a nota e o que mudou em relação à anterior
   (`versions_view::Counts`, contado do `review::diff` ao ler a lista, não a cada quadro: "1 node changed", "2 nodes
   added", "1 other change" para entradas, saídas e o registro do modelo). Uma escolhida: *Review against the model
   open* (o `review::compare`, o mesmo caminho de "outro arquivo": rejeitar uma mudança traz o valor da versão),
   *Restore this version* (`runs_view::restore_named`, um passo de desfazer "Restore the version <rótulo>") e
   *Delete...* (pergunta antes). Duas: *Review the differences* (`review::compare_two`, só leitura). O detalhe de uma
   execução diz "The model of version <rótulo>" quando o modelo dela é o de uma versão. MCP: `save_version` e
   `list_versions` (`tools::save_version`, `tools::list_versions`; no `terraflux mcp` e no Assistente, que guarda o
   modelo da tela; `from: mcp`), um item no guia `results` e uma linha na tabela de `assistants.md`. Testes: um em
   `exec::journal::tests`, dois em `versions_view::tests`, um em `tools`, as asserções no teste do MCP e
   `the_versions_are_saved_listed_reviewed_restored_and_deleted` (ui.rs; a imagem `ui-versions.png`). Ficou de fora:
   os mapas antes e depois na revisão de uma versão (o `compare` não guarda as saídas de antes, como no "outro
   arquivo"), o `terraflux version save|list` na linha de comando e um rótulo repetido (permitido: as versões se
   distinguem pelo id e pela hora).
4. **O espaço em disco antes de rodar, e limpar as saídas.** Antes de uma execução (e de um lote), estimar o que
   será escrito: as células × o tipo × a compressão esperada de cada saída, vezes as iterações dos laços
   (`control.repeat`, `control.each`) e as rodadas do lote; medir o espaço livre do disco de cada pasta de saída
   (Windows, Linux e macOS) e avisar quando não cabe, ou quando sobra pouco, antes de começar e não no fim. O
   `check_model` diz o mesmo. *Clean outputs...* lista as saídas de um modelo (das execuções do diário, as que não
   são mais lidas, as de conferência) com o tamanho de cada uma e apaga as escolhidas, perguntando antes.
   **Feito** (Unreleased): novo `disk.rs` (`space`, `free_space` pelo `GetDiskFreeSpaceExW` no Windows e `statvfs` no
   Linux e macOS, da pasta mais próxima que existe; `fit`: estoura quando passa do livre menos a margem, max(1 GB, 5 %
   do disco), apertado quando sobra menos de 10 %; `plan`: `Go`, `Warn` ou `Ask`; `cost_note`). A estimativa de uma
   execução inteira (`Session::run_bytes`, guardada por revisão do modelo e do diário): `disk::from_journal` (as
   saídas da última execução inteira concluída, a do mesmo modelo, pelo novo `journal::hash_of`, antes) ou
   `disk::from_model` (células do maior raster que o modelo lê, só o cabeçalho, ou `rows` x `columns` dos `nlm.*`;
   4 bytes ou o `dtype`/`type`; as camadas de cada mapa, série ou pilha das saídas e dos `raster.write` (as etapas vêm
   do maior `times`/`steps` do modelo, 1 sem); metade pela compressão; tabelas valem zero). Antes de uma execução
   pedida (`run_all`: o botão, F5 e a paleta; só a do modelo inteiro e salvo, a que grava saídas) e de um lote que
   guarda arquivos (`batch_view::disk_ok`; estimativa por rodada x rodadas; calibração e sensibilidade guardam só
   números, nada) a janela "Not much room on the disk" (*Run anyway*, *Clean outputs...*, *Cancel*); o que cabe mas
   sobra pouco vira uma linha em `Session::notice`; sob o botão do Many runs `cost_ui` acrescenta ", about X on disk
   (Y free)" (em aviso quando estoura). `check_model` e o painel Problems: um aviso do `advice::advise` (nível
   Warning, guia `results`) pela estimativa do modelo, sem o diário (o `advise` só tem a pasta do modelo, não o
   arquivo). *Clean outputs...* (`clean_view.rs`, `Session::open_clean`): menu File, paleta (`Do::Clean`), botão no
   topo do painel Runs e a janela do disco; lista cada caminho uma vez (a execução mais nova que o escreveu; "the
   latest run's" para as da execução mais nova), as pastas dos lotes, tamanho contado numa thread, estado (exists,
   missing, kept); marcados por padrão os lotes e as saídas das execuções anteriores, nunca as da mais nova; pede
   confirmação com a contagem e o tamanho e avisa que é definitivo (sem lixeira); `refuse` recusa o que não é caminho
   completo (ou tem `..`), a raiz do disco, a pasta do usuário (ou acima), a pasta do modelo (ou acima), o arquivo do
   modelo e a pasta `.terraflux`; o diário não é tocado. Para os testes, o campo `Session::disk_space` troca o espaço
   real (para os testes). Testes: cinco em `disk::tests`, três em `clean_view::tests`,
   `the_outputs_of_the_runs_are_cleaned_after_asking` e `a_run_that_may_not_fit_the_disk_asks_first` (ui.rs; as
   imagens `ui-clean-1.png`, `-clean-2-asked`, `-disk-1` e `-disk-2-batch`). Ficou de fora: a estimativa por
   `control.each` e laços aninhados além das etapas (usa o maior `times`/`steps`), "as que não são mais lidas" (a lista é só do diário, não do que o modelo lê agora), a
   estimativa do `terraflux run` (a linha de comando não pergunta) e as execuções do Assistente (`chat.rs` chama
   `Session::start` direto, sem a janela).
5. **Atalhos de teclado configuráveis.** Uma tela no Settings com cada ação (as da paleta de comandos e as do F1),
   o atalho atual e um campo que grava a tecla apertada; conflitos avisados na hora (a mesma tecla em duas ações do
   mesmo lugar: canvas, Viewer, a janela toda), *Reset* por ação e para todas. Guardados no `settings.json`; o F1,
   os menus e a paleta mostram os atalhos escolhidos. Pede tirar os `consume_key` espalhados para uma tabela de
   ações com o atalho padrão de cada uma.
   **Feito, parte 1** (Unreleased): a tabela, a tela e os atalhos da aplicação toda. Novo `keymap.rs`: `ACTIONS`
   (cada `Action` com `id` estável, nome da paleta, `Scope`, atalhos padrão, `fixed` e `idle`), `Shortcut` (com texto
   "Ctrl+Shift+S" gravado e "ctrl+shift+S" / "⌘shift+S" mostrado, `parse`/`Display`), `current`, `text`, `texts`,
   `take` (lê o conjunto de ids de uma vez, o mais específico primeiro, como o egui pede para shift e alt),
   `clashes`/`conflicts` (mesmo `Scope` = erro; global contra um lugar = aviso) e o `Panel` da tela. Os atalhos
   trocados ficam em `Settings::shortcuts` (`settings.json`, lista vazia = sem atalho); `settings::generation()`
   faz o `keymap` refazer o parse só quando as configurações mudam. Passaram para a tabela: `run` (F5), `save`,
   `save_as`, `version`, `new`, `open_model`, `open_data`, `commands` (ctrl+K e ctrl+shift+P), `shortcuts` (F1),
   `to_model` (Tab, não vale digitando), `close_pane` (ctrl+W), `next_tab`, `prev_tab`, `close_tab` (as de
   `Keys::read` em `app.rs` e de `tab_keys` em `tabs.rs`); os menus Arquivo e Ver, a paleta, a dica do botão Run, a
   dica do ponto de "não salvo", a barra de status, a dica da faixa de abas e a seção "Everywhere" do F1 vêm dela.
   Settings ganhou as abas *General* e *Keyboard* (filtro, botão que grava a próxima combinação, Esc cancela,
   Backspace tira, "+" para um segundo, *Reset* por linha e *Reset all*, conflitos na hora, em vermelho para o mesmo
   lugar e laranja para global contra lugar); enquanto grava, o `Panel::capture` pega a tecla no começo do quadro e a
   aplicação espera. Teste: `tests/keymap.rs` (programa próprio: trocar o atalho é de todo o processo, e o `ui` usa
   ctrl+R no canvas). **Parte 2, feita:** as teclas do canvas, do Viewer e dos painéis entraram na tabela (ver
   abaixo).
   **Feito** (Unreleased), parte 2: as teclas do canvas, do Viewer, da calculadora, dos arquivos, do assistente, da
   tabela de atributos e das caixas. Novos `Scope`: `Assistant`, `Table` e `Dialogs` (além de `Canvas`, `Viewer`,
   `Calculator` e `Files`). Mudáveis, lidas pela tabela com `keymap::pressed`/`consume` (dentro de um `InputState`,
   com a condição de foco e de ponteiro de cada lugar como antes) ou `peek`/`peek_exact` (sem tomar a tecla, onde o
   código usava `key_pressed`): canvas `canvas_home` (Home), `canvas_frame` (F, só sem modificadores),
   `canvas_run_to` (ctrl+Enter), `canvas_delete` (Delete e Backspace), `canvas_duplicate` (ctrl+D), `canvas_find`
   (ctrl+F), `canvas_rename` (ctrl+R), `canvas_arrange` (ctrl+L), `canvas_group` (ctrl+G), `canvas_ungroup`
   (ctrl+shift+G), `canvas_bypass` (B), `canvas_select_all` (ctrl+A); Viewer `viewer_satellite` (B), `viewer_hide`
   (espaço), `viewer_measure` (M), `viewer_goto` (ctrl+G), `viewer_back` e `viewer_forward` (alt+setas; os botões do
   mouse seguem fixos), `viewer_step_back` e `viewer_step_next` (setas, a Simulação); calculadora `calc_run`
   (ctrl+Enter) e `calc_complete` (ctrl+Espaço); arquivos `files_back`, `files_forward`, `files_up` (alt+setas) e
   `files_location` (ctrl+L); tabela `table_select_all` (ctrl+A). `fixed` (listadas, só leitura, ainda lidas onde são
   usadas), cada uma pelo motivo: parte da edição de texto ou do jeito do widget (Enter e Esc de caixas e listas, as
   setas em listas e na história do console e do assistente, Tab e Enter dos completamentos, Enter que envia no
   assistente, Esc que para a resposta, ctrl+Enter que salva uma nota, Backspace e Enter e Esc de quem desenha e mede,
   as setas, Enter e Esc dos arquivos e do canvas: `canvas_beside`, `canvas_show`, `canvas_select_none`,
   `viewer_finish`, `viewer_undo_point`, `viewer_cancel`, `calc_next`, `calc_prev`, `calc_accept`, `calc_close`,
   `files_rows`, `files_fold`, `files_enter`, `chat_send`, `chat_stop`, `chat_recall`, `table_none`, `list_down`,
   `list_up`, `box_enter`, `box_escape`, `note_save`); ctrl+C e ctrl+V do canvas, que são eventos `Copy` e `Paste` do
   sistema (`canvas_copy`, `canvas_paste`); e os dígitos dos lugares (`canvas_place_keep`/`_go`,
   `viewer_place_keep`/`_go`: ctrl+1 a ctrl+9 e 1 a 9 seguem os números dos slots, mostrados como "1 to 9"); `undo`,
   `redo` e `add_operator` seguem como estavam (ctrl+Y também refaz, fora da tabela). O F1 sai todo da tabela (uma
   seção por `Scope`, `keymap::of_scope`; só as linhas do mouse ficam como texto, `commands::shortcuts`, e a janela
   ganhou rolagem), e as dicas da barra de status (`canvas_run_to`, `viewer_satellite`, `viewer_hide`,
   `viewer_measure`, `viewer_goto`, `viewer_back`, `viewer_forward`) usam `keymap::text`. Conflitos: mesma regra (mesmo
   `Scope` = erro, global contra lugar = aviso); a ação global que espera durante a edição de texto (`idle`, a Tab) não
   conflita com as `fixed` da edição. Precedência: `Keys::read` (`keymap::take`) deixa para o lugar o atalho global que
   uma ação mudável do lugar sob o ponteiro também tem (`keymap::over(ctx, scope)`, chamado no quadro anterior pelo
   canvas, o Viewer, os arquivos e a tabela, guardado no `Context`; não vale com um campo em foco); o aviso do
   Settings diz quem ganha (se a do lugar é `fixed`, a global vem primeiro). Testes: no `keymap`, os padrões de todas
   contra as teclas de antes, escopos, `fixed`, faixa de dígitos, conflitos e precedência; em `tests/keymap.rs`, F do
   canvas e B do Viewer trocados (a nova age, a antiga não, o F1 e a barra de status mostram a nova, a tela do
   Settings com os escopos em `ui-44-keyboard-scopes.png`) e ctrl+R dado à execução (sobre o canvas renomeia, fora
   dele executa). Fica de fora: as dicas de passar o mouse que citam teclas ("ctrl+enter" no botão Run to, "ctrl+L" no
   Arrange etc.) seguem em texto fixo.
6. **Lugares marcados no Viewer e no canvas.** No Viewer, *Save this place...* guarda a extensão e o zoom com um
   nome (e, opcional, as camadas visíveis); no canvas, a posição e o zoom da câmera. Ctrl+1 a ctrl+9 marcam, 1 a 9
   vão até eles (animado, como o *Go to*), uma lista no menu da vista renomeia e apaga. Guardados com o modelo
   (os do canvas e os do Viewer), para valerem em outro computador; o `set_plot` do MCP pode usar um lugar pelo
   nome.
   **Feito** (Unreleased): novo `terraflux_graph::bookmarks` (`Place { name, slot, view }`, `PlaceView::Map { extent,
   crs, layers }` ou `Canvas { scope, x, y, zoom }`, `SLOTS`) e `Model::bookmarks` (`#[serde(default,
   skip_serializing_if)]`, `[[bookmarks]]` no TOML; `save_bookmark`, `save_bookmark_slot`, `slot_name`,
   `rename_bookmark`, `set_bookmark_slot`, `remove_bookmark`, `bookmarks_of`, `bookmark_at`). Decidido: o campo se
   chama `bookmarks` e não `places`, porque `Model::places` já existe (onde o canvas desenha as caixas do que um
   escopo lê). Um nome repetido de um tipo troca o lugar; um slot é de um lugar por tipo (o Viewer e o canvas têm
   os seus 1 a 9); a tecla guarda como "Place N" (N o slot) ou com o nome que o slot já tinha. Aplicativo: novo
   `places.rs` (`Ask`, `menu_button`/`menu`, `map_ask`, `map_key`, `canvas_ask`, `canvas_key`, `go_map`, `go_canvas`,
   `go_named`, `extent_in`, `read_key`). O menu *Places* no cabeçalho do Viewer (junto de *Go to...*) e como submenu
   do clique direito no canvas vazio: a lista (um clique vai até o lugar; "..." renomeia, dá o slot ou apaga), o
   nome e *Save this place...* (no Viewer, a caixa "with the layers shown"). O menu fica aberto até um clique fora
   (`CloseOnClickOutside`, senão o clique no campo do nome o fechava). Cada gravação, renomeação, slot e remoção é
   uma edição do modelo com o passo "Save the place X", "Rename the place A to B", "Put the place X in slot N",
   "Delete the place X" (ctrl+Z desfaz). Viewer: a extensão é a vista sem cortar no mapa (`MapView::view_extent`,
   `go_extent`, `frame::exact_cells`), então um lugar além da borda volta igual; vai-se até ela de uma vez (o *Go
   to* não anima, então o lugar também não: decidido); em outro CRS a caixa é levada pelo GDAL (`extent_in`, 36
   pontos da caixa) ou recusada dizendo o porquê (mapa sem CRS de um lado só). Teclas: `MapView::place_key` (com o
   ponteiro sobre o mapa e nenhum campo em foco, lido na hora de desenhar e aplicado em `builder::preview`) e em
   `builder::keys` para o canvas (com o ponteiro sobre ele); ctrl+1 a 9 guardam, 1 a 9 vão; alt e shift sozinhos não
   valem. No F1 (Maps e canvas) e na paleta ("Go to place 2: Lake", `Do::Place`; o foco vai para o Viewer ou o
   modelo). MCP: `set_plot` com `place` (a extensão de um lugar do mapa, na CRS do lugar; `view` explícito vence) e o
   `show_model` com uma linha por lugar. Testes: três em `graph::bookmarks::tests`, quatro em `places::tests`, um em
   `frame::tests`, `a_plot_takes_the_view_of_a_place` (tools.rs) e
   `places_are_kept_with_the_model_and_gone_back_to` (ui.rs; as imagens `ui-places-menu.png` e
   `ui-places-canvas-menu.png`). Ficou de fora: uma animação da câmera, o lugar do canvas só no escopo do contêiner
   (o escopo some do modelo: o aviso diz), as camadas do lugar só pelas que já estão na lista do Viewer (as que o
   lugar guardou e a lista não tem não voltam) e um *Go to* de lugar no Viewer sem mapa.

## A análise de sensibilidade no aplicativo

Pedido de 2026-11-06: "seria legal pensar em um modo que facilite a implementação de análises de sensibilidade nos
modelos. Porque é algo que é bastante comum", e depois "quero que registre no IDEAS.md sim. E depois pode começar a
implementar na melhor ordem". O motor já existe (0.94.0, item 6 de "O que ainda falta entre os operadores"):
`terraflux_ops::sensitivity` (Morris e Sobol, iguais ao SALib 1.6), os nós `sensitivity.design` e
`sensitivity.analyze` e o `terraflux sensitivity` (`crates/cli/src/cluster.rs`), que monta o desenho, roda nos lotes
do `terraflux_dist` (aqui, em clusters, na nuvem) e mede uma saída (`Evaluation::measure`, com `statistic` mean, sum,
min ou max de um mapa). Falta tudo na interface: só quem usa a linha de comando faz a análise.

1. **A aba Sensitivity no Many runs** (`batch_view.rs`, um quarto `Kind` ao lado de réplicas, cenários e calibração),
   com a tabela de faixas da calibração: cada fator um parâmetro do modelo (`$entrada` ou `nó.configuração`) com
   mínimo e máximo, a faixa padrão ±20 % do valor atual (os inteiros arredondados). Três métodos: **OAT** (um fator
   por vez, do mínimo ao máximo com a base no resto: o gráfico tornado, o mais pedido em ecologia; novo em
   `terraflux_ops::sensitivity`, também no `terraflux sensitivity --method oat`), **Morris** (a triagem) e
   **Sobol** (a variância repartida, S1 e ST com intervalos por bootstrap). O texto da aba sugere o caminho: Morris
   primeiro, Sobol nos fatores que importaram. Os resultados na aba: a tabela dos índices e o CSV do desenho e dos
   índices, como os do `terraflux sensitivity`, numa pasta escolhida.
   **Feito** (Unreleased): `terraflux_ops::sensitivity::oat_sample` (2k+1 linhas: a base no meio das faixas e cada
   fator no mínimo e no máximo) e `oat_analyze` (`Oat { base, low, high, swing }`); o módulo novo
   `terraflux_dist::sensitivity` (`Method`, `Design`, `runs`, `rows`, `candidates`, `analyze`, `Indices`,
   `design_csv`, `indices_csv`, `s2_csv`, `write_design`, `write_indices`), que o `terraflux sensitivity` agora usa
   (também com `--method oat`; as colunas do OAT são `factor,low,high,swing,base`) e a aba. No `batch_view.rs`:
   `Kind::Sensitivity`, `SensitivityState` (`BatchState::sens`: fatores, método, `Design`, bootstraps, saída,
   estatística), `sensitivity_ui`, `factors`/`sensitivity_runs` (o botão "Run N times" sem montar o desenho),
   `default_range` (±20 %, inteiros ampliados, vazio sem valor), `Knob::numeric/integer/default` (só números viram
   fatores; o padrão do operador vem de "default N" na descrição), `measurable`/`default_measure`, o
   `SensitivityPlan` no `Record` (`#[serde(default)]`: o lote retomado ainda dá os índices), `Finished::sensitivity`
   e a tabela dos índices (OAT por swing, Morris por mu*, Sobol por ST). Testes: `oat_runs_the_base_then_each_factor_at_its_ends` e
   `oat_swing_of_a_linear_output` (ops), os quatro de `dist::sensitivity::tests`, `a_factor_starts_20_percent_either_side`
   e `a_sensitivity_analysis_runs_from_its_pane` (a imagem `ui-batch-6-sensitivity.png`; OAT de 2 fatores, 5
   rodadas no Sibuyan). Falta: os itens 3 a 6.
2. **O que medir, sem montar nós.** A saída é escolhida entre as do modelo (`nó.porta`); de um mapa, um resumo
   pronto: média, soma, mínimo, máximo, a área (ou a fração) de uma classe, a fração das células que mudaram em
   relação à rodada base, uma métrica de paisagem da classe. Os resumos novos em `terraflux_dist::evaluate` (o
   `statistic`), valendo também no `terraflux sensitivity` e na calibração.
   **Feito** (Unreleased): `evaluate::measure_from` (`measure` continua, sem o início) com `cells:<c>`, `share:<c>`,
   `area:<c>` (hectares quando `hectares_per_cell` sabe que a grade é projetada em metros, pelas GeoKeys ou pelo
   WKT; senão unidades do mapa ao quadrado), `changed` (primeiro contra último de série ou pilha; de um mapa só,
   contra o `Evaluation::start`, que `evaluate` acha nas entradas ou nos resultados, e sem início um erro que manda
   escolher o mapa inicial) e `metric:<np|pland|ed|area_mn|lpi>:<c>` (`ops::metrics::measure` com
   `default_options`; classe ausente: np 0, o resto NaN). No `batch_view.rs`: `SensitivityState::{class, metric,
   start}`, `statistic_of` (agora monta o texto e recusa classe que não é número) e as linhas Of the map / Metric /
   Class / Against de `sensitivity_ui`; ajuda de `--statistic` em `cluster.rs`. Testes: os quatro de
   `dist::evaluate::tests`, `a_maps_statistic_is_composed_from_its_kind_and_class` e `share:` no final de
   `a_sensitivity_analysis_runs_from_its_pane`. Ficou de fora: a calibração (não usa esses resumos). Falta: os itens 3 a 6.
   **Feito (2026-11-14): os nomes das classes na aba.** `Class` é um menu "valor – nome" (`batch_view::class_choices`:
   a legenda de um `raster.legend` a montante, via `legend_view::declared`, ou as classes do produto da paleta, como
   o MSPA) com "Other value..." e o botão Names de volta; sem legenda, ou com valor fora dela, o campo numérico de
   antes. `SensitivityState::class` segue o número; o texto de `methods.md` diz "forest (1)" (`class_label`).
3. **O custo antes de rodar.** As rodadas do desenho (OAT 2k+1, Morris r(k+1), Sobol N(2k+2) ou N(k+2)) vezes as
   réplicas, e o tempo pelo `eta.rs` (a soma dos tempos dos nós das execuções anteriores) dividido pelos workers:
   "Sobol of 6 factors, N=256: 3 584 runs, about 5 h here". Um aviso quando passa de algumas horas.
   **Feito** (Unreleased): no `batch_view.rs`, `batch_time` (rodadas = ceil(rodadas / ao mesmo tempo) × uma rodada),
   `one_run_time` (a soma dos tempos dos nós; `times_before` com `timings` da sessão por cima; nenhum sem tempos),
   `cost_text` (a linha, "about", e se passa de 2 h) e `cost_ui` sob o botão de todos os tipos (aqui: os workers do
   controle; num cluster salvo: nós × workers do `Scheduler`; cor `warn_fg_color` e a dica, de Morris primeiro na
   sensibilidade, de menos rodadas ou mais computadores nos outros). As réplicas (item 4) entram na conta.
   Testes: `a_batch_takes_rounds_of_runs_at_once` e `the_cost_of_a_batch_is_told_before_it_runs` (ui.rs). Falta: os
   itens 4 a 6.
4. **Modelos estocásticos: réplicas em cada ponto do desenho.** Com um seed no modelo (`batch::seed_keys`), cada
   ponto roda com R seeds (padrão 1; o texto sugere 3 a 5 num simulador de mudança de uso) e a saída é a média das
   réplicas; a variância entre as réplicas é mostrada ao lado da dos fatores, para dizer quanto do resultado é
   acaso e não o parâmetro.
   **Feito** (Unreleased): no `terraflux_dist::sensitivity`, `seeds` (o seed do modelo e os seguintes), `total_runs`
   (rodadas do desenho × R), `replicated` (por ponto, a média das réplicas que rodaram, o desvio padrão `sd`, quantas
   rodaram e quantas falharam; um ponto sem nenhuma réplica é NaN) e `Replicated::chance` (a variância média entre
   as réplicas de um ponto sobre a variância de todas as saídas, de 0 a 1; nada com R = 1). `Indices` ganha
   `replicates` e `chance` (`#[serde(default)]`: registros antigos ainda abrem), e `design_csv`/`write_design`
   recebem o `Replicated` (com R > 1, as colunas `sd` e `n`). As rodadas vêm de `batch::calibration_tasks` com a
   lista de seeds (candidato por candidato, as réplicas em sequência, como na calibração). O `terraflux sensitivity`
   tem `--replicates` (padrão 1) e escreve o acaso no fim. No `batch_view.rs`: `SensitivityState::replicates`,
   `replicates_of` (1 sem seed no modelo), o campo "Replicates at each point" (desligado sem seed) com a dica,
   `SensitivityPlan::replicates` (`#[serde(default)]`), a contagem e o `cost_ui` com as réplicas ("..., 3 replicates:")
   e, em `finished_ui`, "Chance: N % ..." (cor de aviso acima de 50 %). Testes: `the_replicates_of_a_point_are_averaged`,
   `the_chance_is_the_share_of_the_variance_among_the_replicates` e `a_failed_replicate_is_left_out_of_the_mean`
   (dist) e `a_sensitivity_analysis_runs_replicates_at_each_point` (ui.rs; a média do mapa não muda com o seed, por
   isso o teste mede `metric:np`). Falta: os itens 5 e 6.
5. **Os fatores marcados no modelo.** No painel Node, o clique direito numa configuração numérica dá *Vary in a
   sensitivity analysis* (abre a aba com o fator posto, ±20 %); um parâmetro do modelo também. A aba propõe como
   fatores as configurações citadas nas `assumptions` dos nós (o que foi suposto é o que mais vale variar) e os
   parâmetros do modelo com faixa (`min`/`max`).
   **Feito** (Unreleased): `Session::ask_vary` (chave e faixa) leva o pedido do painel Node e do Parameters ao app,
   que o entrega a `batch_view::vary` (põe o fator uma vez, faixa dada ou ±20 %, muda para `Kind::Sensitivity`) e traz o
   Many runs à frente. Node: `vary_key` e o item *Vary in a sensitivity analysis* no menu de `name_cell` (número, ou
   `$entrada` numérica quando a configuração lê `← $x`). Parameters: o botão *Vary* em cada número (faixa `min`..`max`
   se há as duas) e *Sensitivity of these parameters...* (os com faixa primeiro). Na aba: `batch_view::suggestions`
   (parâmetros com `min` e `max`, depois configurações numéricas citadas, como palavra inteira, nas `assumptions` do
   próprio nó, e `$entradas` citadas nas de qualquer nó; `_` vale espaço) e a lista *Suggested* com "+", a dica do
   porquê, *Add all* e "and N more" depois de 8. Testes: `the_model_suggests_its_ranged_parameters_and_assumed_settings`
   (batch_view), `the_model_suggests_the_factors_it_assumes` e `a_setting_is_varied_from_its_menu_and_the_parameters_pane`
   (ui.rs, com o clique direito no kittest). Falta: o item 6.
6. **Os gráficos e o registro.** O tornado (OAT), o μ*–σ (Morris, os fatores acima da diagonal não lineares ou com
   interações), as barras de S1 e ST com os intervalos (Sobol), e a convergência (os índices com metade, um quarto
   das rodadas: estáveis ou não, um aviso quando não). Exportados como PNG e CSV; a análise vai para o diário das
   execuções (item 1 da seção anterior) e o `methods.rs` escreve o seu parágrafo (o método, o desenho, N, as
   réplicas, a referência do SALib, de Morris 1991, Saltelli 2002/2010). No MCP, uma ferramenta `sensitivity` que
   monta e roda a análise e devolve os índices.
   **Feito em parte** (Unreleased): os gráficos, a convergência e os arquivos. `terraflux_dist::sensitivity` ganha
   `Convergence` (`Indices::convergence`, `#[serde(default)]`: por fator, μ* ou ST com um quarto, metade e todo o
   desenho, em trajetórias inteiras (Morris, a partir de 2) ou blocos inteiros de pontos-base (Sobol, a partir de 4);
   o OAT não tem), `convergence_csv`, `TOLERANCE` (0,05) e `MATTERS` (0,1): estável quando, de metade para todo o
   desenho, nenhum fator mexeu mais que 5 % do maior valor e os fatores acima de 10 % do maior mantêm a ordem;
   `write_indices` escreve também o `convergence.csv` (vale para o `terraflux sensitivity`). Novo
   `crates/app/src/sens_chart.rs`: `show` (egui_plot em `finished_ui`: o tornado do OAT, o μ*–σ do Morris com as
   guias σ/μ* = 0,1 e 1 e a dica de Garcia Sanchez et al. 2014, as barras de S1 e ST com os intervalos do Sobol; sob
   elas o veredito e um gráfico de linhas dos valores em cada tamanho), `verdict`, `drawing` (o mesmo gráfico como
   `figure::Drawing`, com o texto em contornos, pelo pequeno construtor novo `Drawing::{blank, rect, polyline, dot,
   label}` e `text_width` em `figure.rs`), `write` (`sensitivity.svg` e `sensitivity.png` na pasta, ao acabar a
   análise) e `save` (o botão *Save the chart...*, SVG ou PNG). Testes: quatro em `dist::sensitivity::tests`
   (linear estável com poucas trajetórias, ruidoso instável, Ishigami com N = 2048 estável e ST perto de 0,56/0,44/0,24,
   estrutura com N pequeno), três em `sens_chart::tests`, e no ui.rs os arquivos em `a_sensitivity_analysis_runs_from_its_pane`
   e `..._replicates_at_each_point` e o novo `a_morris_analysis_checks_its_convergence` (as imagens `ui-batch-8-morris-done.png`
   e `ui-batch-9-oat-chart.png`).
   **Feito também** (Unreleased): o parágrafo dos métodos e a ferramenta do MCP. Novo `crates/app/src/sens_methods.rs`
   (`Run`, `paragraph`, `text`, `write`, `references`): do método, desenho, fatores e faixas, saída e estatística,
   réplicas, acaso e convergência faz o parágrafo em inglês (as frases das réplicas, do acaso e da convergência só
   aparecem quando valem) e as referências do método; vira `methods.md` na pasta ao fim da análise (aplicativo e
   `terraflux sensitivity`) e o botão *Copy the methods text* do `finished_ui`. As sete referências estão em
   `methods::REFERENCES` e no `citing.md` (uma linha na tabela, a seção *Sensitivity analysis* e a lista), e o
   `METHODS` ganha `sensitivity.design`/`sensitivity.analyze` (SALib). `SensitivityPlan` guarda `bounds` e
   `statistic` (`#[serde(default)]`; um lote retomado de antes não escreve o `methods.md`). No `cluster.rs` o
   `terraflux sensitivity` agora é `Spec` → `plan` → `execute` (devolve `Analysis`: índices, arquivos, texto dos
   métodos; escreve também o gráfico), reaproveitado pelo MCP. Ferramenta `sensitivity` (`tools.rs`: `sensitivity_ask`,
   `SensitivityAsk`, `BIG_DESIGN` = 500; `mcp.rs`: `sensitivity`, numa thread como o `run_model`, com os workers daqui
   limitados a 4 e o progresso/cancelamento do `Watcher`): fatores `{key, from, to}` (sem faixa, ±20 %:
   `batch_view::asked_ranges`), método, n, levels, second_order, replicates, saída (padrão `default_measure`),
   statistic, out (padrão `<modelo> runs/sensitivity N`), `confirm` (acima de 500 rodadas recusa e diz a conta);
   responde com os índices, o acaso, o veredito da convergência, o parágrafo e os arquivos. No assistente do
   aplicativo a mesma ferramenta não roda: `chat::set_up_sensitivity` põe fatores e configurações no Many runs →
   Sensitivity (`Session::ask_sensitivity`, `batch_view::set_asked`) e manda o usuário apertar Run. Guia `results`
   ganha o parágrafo de como usar (Morris primeiro, Sobol no que importou, réplicas, registrar nas notas). Testes:
   cinco em `sens_methods::tests`, `tools::sensitivity_tests` (2), `an_analysis_asked_for_by_the_assistant_is_put_in_the_tab`,
   `a_sensitivity_analysis_is_run_and_told` e `a_big_sensitivity_design_is_refused_until_confirmed` (mcp), e o
   `methods.md` e o botão em `a_sensitivity_analysis_runs_from_its_pane` e o assistente em
   `a_setting_is_varied_from_its_menu_and_the_parameters_pane`. O registro da análise no diário das execuções
   também está feito (item 1 da seção anterior: uma entrada de lote, com os fatores ordenados). Falta do item 6: a
   análise vista em detalhe a partir do painel Runs.
7. **Mapas de sensibilidade.** De uma saída em mapa, por célula: o desvio padrão entre as rodadas do desenho (onde
   o resultado é incerto) e, com Sobol, o S1 e o ST de cada fator por célula (onde cada fator manda), como uma
   pilha que o Viewer mostra. Pede guardar os mapas de cada rodada (ou acumulá-los nos workers): o mais caro, o
   último.
   **Feito** (Unreleased): `terraflux_dist::sensitivity::{map_bands, average_replicates, cell_bands}` (a conta, pura:
   `y[ponto][célula]` vira uma banda por `mean`, `sd` e, por fator, o `swing_` do OAT, o `mu_star_` do Morris, o `s1_` e
   o `st_` do Sobol; sem bootstrap por célula (`resamples = 0`, as funções do `ops` aceitam); NaN onde um ponto não tem
   valor, 0 numa célula que não muda; em rayon sobre as células) e o novo `terraflux_dist::sens_maps` (`Spec`, `make`,
   `make_within`, `block_rows`, `run_file`, `keep_last_map`, `Made`, `FILE`: abre o mapa de cada rodada, lê por blocos
   de linhas com `Image::read_window` (as células de todas as rodadas de um bloco em até 256 MB), as células sem valor
   do arquivo viram NaN, escreve `sensitivity_maps.tif` em float32 com nodata NaN e as bandas nomeadas, e só então apaga
   os mapas das rodadas, salvo `keep_runs`). Cada rodada guarda só a última camada da saída medida
   (`Output::layer = "last"`, novo em `ops::dynamic::layer_of`, vale também no `stack.extract_layer`) e as saídas do
   modelo ficam de fora. No `batch_view.rs`: `SensitivityState::{maps, keep_run_maps}`, `maps_ui` (a caixa *Maps of
   sensitivity, cell by cell* para saída mapa, série ou pilha, a conta do disco em rodadas x um mapa e *Keep each run's
   map*), `maps_of`, `batch_bytes` (a linha de custo e o aviso de disco contam os mapas, `disk::map_bytes`),
   `SensitivityPlan::maps` (`MapsPlan`, `#[serde(default)]`: o lote retomado também faz os mapas), `Making` e
   `make_maps` (uma thread; o progresso em milésimos, "making the maps of sensitivity: 40 %" no `finished_ui`),
   `poll_maps` (põe o `Made` em `Finished::maps` e reescreve o `methods.md` com a frase nova de `sens_methods::Run::maps`),
   e em `finished_ui` o arquivo, as bandas e *Show in the Viewer* (`drop_on_viewer`). `terraflux sensitivity --maps` e
   `--keep-run-maps` (`cluster::MapsArgs`, numa struct à parte: duas flags a mais direto em `Sensitivity` estouraram a
   pilha da thread principal do build debug) e `maps`/`keep_run_maps` na ferramenta `sensitivity` do MCP
   (`SensitivityAsk`, `Spec`, `Analysis::maps`); o assistente do aplicativo põe a caixa no `set_asked`. O
   `sensitivity_command` saiu do `main` pelo mesmo motivo. Testes: os de `dist::sensitivity::tests` e
   `sens_maps::tests` (metades esquerda e direita do mapa, ST/swing/mu*, NaN, réplicas, bloco que não divide as linhas,
   rodada que falhou, apagar ou guardar os mapas), `last_names_the_last_layer...` (ops),
   `the_maps_of_sensitivity_are_told_only_when_made`, `a_plan_kept_before_the_maps_still_opens_and_keeps_them_after` e
   `a_map_kept_by_a_run_is_one_layer...` (app), `a_sensitivity_analysis_makes_maps_of_sensitivity` (ui.rs: OAT de 2
   fatores no Sibuyan, 4 bandas, mapas das rodadas apagados e guardados, o Viewer; imagens
   `ui-batch-10-sensitivity-maps.png` e `ui-batch-11-sensitivity-maps-viewer.png`) e `maps: true` em
   `a_sensitivity_analysis_is_run_and_told` (mcp). Ficou de fora: a pilha inteira fica na memória (bandas x células x 8
   bytes) até a escrita, só a leitura é em blocos; o Viewer abre a pilha com a banda `mean` à frente (as outras pelo
   seletor); os nós `raster.write` do modelo ainda escrevem na pasta de cada rodada; os mapas não entram no diário.

**A ordem (2026-11-06)** das duas seções, as que dependem de outras depois delas: a sensibilidade 1, 2 e 3 (a
análise inteira no aplicativo com o motor que existe), 4 (réplicas), 5 e 6 (sem o diário); depois o diário das
execuções (1 da seção anterior), as versões com nome (3, guardadas com o diário), e o registro da análise no
diário; o espaço em disco e *Clean outputs* (4); a execução de conferência (2, que mexe no `exec`); os mapas de
sensibilidade (7); os lugares marcados (6); os atalhos configuráveis (5, por último: tira os `consume_key`
espalhados para uma tabela). Cada item: testes (unitários e, quando há interface, do kittest), clippy, fmt,
CHANGELOG (Unreleased), README quando muda o que o usuário faz, a marca **Feito** aqui, um commit local; release só
quando pedido.

## Os ajustes no próprio nó: menos dependência da aba Node

Pedido de 2026-11-15: "colocar os parâmetros mais comuns/usados/necessários no próprio node como em programas como a
Unreal Engine 5 faz. E só abrir a aba de nodes para os nodes que realmente precisarem de mais parâmetros", para dar
mais espaço ao canvas e ao Viewer. Depois, "gostei das ideias. O desempenho da interface não pode ser ruim... tem que
continuar sendo 100% responsiva" e "pode implementar as ideias tb". Um commit por item; release só quando pedido.

O que já existe e é aproveitado: a lista dos ajustes essenciais (`terraflux_ops::essentials`, 30 operadores), a
linha "+ N optional" que dobra as entradas opcionais, o ajuste que lê outro valor (`$nome`, `@nome`,
`{ from = "no.porta" }`, `setting_ref`), a cena do canvas feita só quando o grafo muda (os textos só das caixas em
vista). Um ajuste "dado" é o que está no `params` do nó (o modelo guarda só os dados; voltar ao padrão o tira):
contar os dados não pede um padrão no schema.

**A regra do desempenho, para todos os itens.** Nada novo por quadro que cresça com o modelo: os textos das linhas
(nome, valor cortado, padrão) feitos na montagem da cena (`Scene::build`, a cada edição), nunca por quadro; desenhados
só nas caixas em vista e do zoom 0.6 para cima (como os nomes das portas); no máximo 4 linhas de ajustes por caixa,
o resto na linha que dobra; um só campo de edição aberto por vez (os outros são texto pintado); as bolinhas dos
ajustes pelo GPU com as portas. Medido antes e depois com `canvas::timings` (100, 1.000 e 10.000 nós; `--release
--ignored`): os quadros de perto, de longe, arrastando um nó e o tempo de uma edição não podem piorar de forma
sensível (até cerca de 10 %, dentro do ruído).

1. **Os ajustes à vista no nó.** Abaixo das entradas, uma linha por ajuste, o nome à esquerda e o valor à direita
   (cortado à caixa; um ajuste não dado, o padrão do seu doc em cinza): os que o operador precisa, os dados e os
   essenciais, no máximo 4; os outros dobrados em "+ 5 settings · 2 given" (nunca um ajuste mudado escondido sem
   aviso). Um ajuste com problema (o check) com a linha em vermelho ou âmbar. A altura da caixa conta as linhas
   (o arranjo e o layout automático também). Uma opção do canvas, *Settings on nodes* (ligada), guardada nas
   preferências, para quem quer as caixas de antes. **Feito** (Unreleased): `editor_view::node_settings`
   (`choose`, `MAX_SETTINGS`), os textos feitos em `Scene::build`, uma só conta da altura (`box_height`, usada pelo
   arranjo e pelo layout automático), a opção em *Settings* ("a node's main settings on its box"), as linhas de
   erro do check (`at = "no.ajuste"`; o aviso "does not read the setting" não diz qual e fica sem marca).
   Medido: os quadros iguais, uma edição de 10.000 nós igual (os textos monoespaçados medidos pela conta das letras),
   arrastar um nó de 10.000 cerca de 6 % mais lento (as caixas mais altas desviam mais fios). Teste:
   `a_nodes_box_shows_its_settings`.
2. **Editar no nó.** Um clique no valor: um sim/não troca; uma lista de escolhas abre o menu ali; um número ou texto
   abre um campo no lugar do valor (Enter grava, Esc desiste; um passo de desfazer). Clicar na linha que dobra
   desdobra todos os ajustes. Arquivos, listas, tabelas e scripts não são editados na caixa: abrem o painel do item 3.
   **Feito** (Unreleased): `editor_view::Kind` (decidido na montagem da cena), `Action::SetParam` pelo mesmo
   `editor::set_param` do painel, um só editor aberto (`EditorState::editing`), Tab para a próxima linha de texto, os
   atalhos do canvas parados enquanto se digita; um duplo clique numa linha editável não abre o Viewer. Medido: sem
   diferença nos quadros nem na edição. Testes: `a_yes_or_no_and_a_choice_are_set_on_the_box`,
   `a_number_is_typed_on_the_box`. **Feito também** (Unreleased): um clique na linha de uma lista aberta a fecha
   (`EditorState::choice_closed`: o clique que segue o aperto que fechou a lista não a reabre); teste
   `a_click_on_the_row_of_a_choice_toggles_its_list`.
3. **O painel ao lado do nó.** O painel do nó (o mesmo da aba Node) num quadro flutuante junto da caixa, aberto pelo
   ⚙ da caixa ou por um ajuste que não se edita nela, fechado por um clique fora ou Esc. A aba Node deixa de abrir
   sozinha: continua no menu de painéis para quem a quer sempre aberta. **Feito** (Unreleased):
   `builder::settings_panel` (o `node_view::panel` numa janela presa à caixa, à direita ou à esquerda, que segue a
   vista e a seleção), aberto pelo ⚙ da linha do operador, por um ajuste que não se edita na caixa, por shift+Enter
   (`canvas_settings`, configurável: Enter já mostra o que o nó dá) e por *Settings...* no menu do nó; fechado por ×,
   Esc, um clique no canvas vazio ou a troca de escopo. A preferência "open the Node pane by itself" (desligada).
   Testes: `a_nodes_settings_open_in_a_panel_beside_it`, `the_panel_opens_from_a_row_enter_and_the_menu`,
   `the_node_tab_opens_by_itself_only_when_asked`. **Feito também** (Unreleased): o painel rola até o ajuste clicado
   (`Action::OpenSettings(nó, Some(ajuste))`, `EditorState::panel_focus`; `node_view::setting_marked` rola até a linha
   nos primeiros 0,3 s, abre o "Advanced" se o ajuste está nele e o marca com um contorno que some em 1,6 s; pela
   engrenagem, sem ajuste, fica no topo); teste `the_panel_opens_scrolled_to_the_setting_clicked`.
4. **O ajuste como pino.** Cada linha de ajuste tem uma bolinha à esquerda, como uma entrada: um fio solto nela faz
   o ajuste ler aquela saída (`{ from = "no.porta" }`, ou `$nome` de uma entrada do modelo), o valor dá lugar ao fio;
   tirar o fio volta ao valor. Botão direito numa linha: *Show on the node* / *Hide from the node* (guardado no nó,
   `shown`/`hidden`), por cima da regra do item 1. **Feito** (Unreleased): a bolinha nas linhas de número, inteiro,
   sim/não e texto (`SettingRow::feed`), os fios dos ajustes numa lista própria (`Scene::setting_wires`, seguindo as
   caixas pelos mesmos caminhos dos outros fios, sem mexer nos das entradas), `editor::connect_setting` checado como
   uma conexão, `Action::ConnectSetting`/`ShowSetting`, *Use a value of its own*, *Hide from the node* e o submenu
   *Show on the node*; `Node::shown`/`hidden` fora do cache, do que fica velho e da comparação do arquivo seguido (só
   desenho). Medido: sem diferença. Testes: `a_wire_dropped_on_a_setting_makes_it_read_the_value`,
   `a_setting_is_hidden_from_the_box_and_shown_on_it`. **Feito também** (Unreleased): (1) a dica ao passar o ponteiro
   num fio de ajuste ("→ no.ajuste", o tipo e o último valor, como a de uma conexão: `WireAsk::into`); (2) arrastar da
   bolinha de um ajuste livre até uma saída que o alimenta (`Drag::FeedSetting`, o mesmo `Action::ConnectSetting`);
   (3) pontos de dobra próprios, sem mudar o formato do modelo: em `Node::wires` sob a chave `setting:<nome>` (nenhuma
   porta tem dois-pontos no nome), `editor::setting_wire`; `add/move/remove_wire_point` aceitam a chave, `prune_wires`
   e `shift_wires` a conhecem, `set_param`/`connect_setting` a apagam (os pontos vão com o fio); na cena os fios dos
   ajustes seguem os das conexões na numeração (`Scene::wire_into`, `via_mut`). Medido (`canvas::timings`, antes →
   depois, 100/1.000/10.000 nós, mediana): de longe 92 µs → 90, 137 µs → 139, 856 µs → 849; de perto 114 → 116, 195 →
   205, 843 → 758; arrastando 333 µs → 332, 2,70 ms → 2,80, 30,8 ms → 32,1; uma edição (conferida) 1,41 → 1,39 ms, 3,88
   → 3,99, 32,8 → 39 (ruído da máquina; o grafo do teste não tem fios de ajuste); teste
   `a_settings_wire_has_a_tip_points_and_can_be_drawn_from_the_socket`.

## Qualidade de vida: corrigir, compartilhar, figurar, substituir e prever

Pedido de 2026-11-16 (um commit por item, na ordem 1, 5, 3, 2, 4; o tema que segue o sistema ficou de fora, por
escolha do usuário). Conferido antes que nada disso existe.

1. **Correção com um clique no painel Problems.** Hoje o painel só oferece "Ask the assistant" e "Copy". Os
   problemas comuns ganham um botão que os resolve, passando pelo desfazer: mapas em CRS ou grades diferentes → um
   `raster.match` inserido; um mapa sem nodata → o nodata definido; uma entrada vazia → o seletor de arquivos.
   **Feito** (Unreleased). `crates/app/src/fixes.rs`: `fix_for(model, registry, problema) -> Vec<Fix>` e `apply`
   (numa cópia; recusa o que não se aplica mais ou traz erro novo), `run` na sessão como um passo de desfazer; o painel
   (`problems_view::fix_ui`) só chama. Mapas de grades diferentes (a falha explicada): um `raster.match` antes de cada
   entrada que destoa da primeira, `near` se as células são inteiras (`Side::classes`, amostra), `bilinear` se são
   contínuas, dois botões se não se sabe. Nodata: campo para digitar + Fix, nas entradas `$x` dos achados do lint
   ("check its no-data value") e no `raster.read` que a falha cita. Entrada vazia: "Use $x" para as entradas do modelo
   do tipo da porta, e Fix abre o `rfd` e liga um `raster.read`/`vector.read`/`table.read`. Ficou: o valor de nodata
   que o arquivo declara não é lido (o usuário digita); nada hoje acusa um `raster.read` sem nodata antes de uma falha;
   o id citado que não existe não tem correção.
5. **Empacotar o modelo com os dados.** Um arquivo só (o modelo, os dados, os caminhos relativos, o manifesto) para
   o orientador ou o revisor; "Abrir pacote" desempacota e o modelo roda sem ajuste.
   **Feito** (Unreleased): `.tfpack` (um zip, escrito por `terraflux_exec::pack` e `zipfile`, deflate e ZIP64, sobre o
   `flate2` que já estava no build), com o modelo (todos os caminhos relativos em `data/`), os arquivos lidos (irmãos
   do shapefile, pastas e padrões, submodelos com os dados deles, plugins), as saídas da última execução se pedido e
   o `manifest.json` (versão, data, SHA-256, tamanho e caminho original). Nomes iguais não colidem (`data/2/`); o que
   falta é listado; o total aparece antes de gravar e passa de 2 GB com aviso. *File > Pack for sharing...*, *File >
   Open package...* (e soltar o arquivo na janela) e `terraflux pack`/`unpack`; o desempacotado dá as mesmas saídas.
   Fica para depois: a ferramenta `pack_model` do MCP.
3. **O grafo do modelo como figura vetorial** (SVG e PDF) para o artigo, no tema claro, com as notas. **Feito**
   (Unreleased): File > "Export the model as a figure..." (paleta, e "Export as a figure..." no menu de uma seleção ou
   de um grupo) e `terraflux figure modelo -o grafo.svg --width 85mm`; `graph_figure.rs` monta o desenho no
   `figure::Drawing` dos mapas (SVG, PDF e PNG saem do mesmo layout), com as caixas, portas, fios, grupos abertos,
   notas, molduras e linhas de ajustes do canvas, sempre no tema claro (texto a 4.5:1, linhas a 3:1, testado).
2. **Localizar e substituir no modelo inteiro**, com prévia: um caminho, um nodata, o nome de uma classe em todos
   os nós de uma vez (hoje só nos nós selecionados do mesmo operador). **Feito** (Unreleased): ctrl+H, o menu Edit e
   a paleta abrem a janela "Find and replace" (`crates/app/src/replace.rs`: `find` e `replace` puros, a janela só os
   chama; ação `find_replace` no keymap); procura nas configurações de todos os nós em qualquer profundidade, nos
   caminhos, nos scripts e nas regras de reclassificação, nas notas e nos nomes/arquivos das entradas e saídas, com
   maiúsculas, palavra inteira e expressão regular (`$1`), filtros por onde e por operador; a prévia lista cada
   ocorrência com caixa de seleção e vai ao nó ao clicar; "Replace selected" é um passo de desfazer, tudo ou nada,
   conferido como uma edição, números escritos de volta como números, e depois avisa os arquivos que não existem.
   Testes: `replace::tests`, `find_and_replace_previews_unchecks_and_applies_in_one_step`.
4. **A memória estimada antes de rodar** (o disco já é estimado; a memória de cada nó só é medida depois): "esta
   execução precisa de ~12 GB e há 8", com a sugestão de rodar por blocos ou por tiles.
   **Feito** (Unreleased). Novo `crates/app/src/memory.rs`: `ram()` (total e disponível: `GlobalMemoryStatusEx` no
   Windows, `/proc/meminfo` no Linux, `sysctl` no macOS, onde o disponível é só o das páginas livres; `pretend` para os
   testes, por thread) e `estimate(model, registry, dir) -> Estimate`: um mapa na memória são 8 bytes por célula; a
   grade de cada nó vem do cabeçalho do arquivo (`raster.read`, entradas do modelo), de `rows` x `columns` (`nlm.*`)
   ou da entrada principal (`like`, `map`, `raster`, `stack`); pilha = camadas, série de laço = um mapa por passo,
   tabela e vetor = o tamanho do arquivo (vetor x 2). O pico é uma cota superior: `kept` (saídas dos nós de nível
   superior, que o cache segura todas) + `reads` (mapas das entradas e rasters lidos inteiros; zero se só quem os lê
   trabalha em blocos: `block_rows`, `landscape.mspa_tiled`, métricas de mapas de mais de 250 milhões de células) +
   `bodies` (o corpo dos laços: uma iteração x passos, no máximo 1 GiB, o que o cache solta) + `scratch` (a maior
   saída de novo). A medida de uma execução anterior do mesmo modelo (`Entry::memory` no diário, a soma do que o runner
   mede) substitui o `kept` e o texto diz "from the run before". Em 15 templates que rodam, o `kept` estimado fica entre
   0,45x e 1,3x do que o motor segura (a maioria dentro de 5 %); o que ele não vê são vetores e tabelas feitos por
   operadores. Acima de 80 % do disponível: aviso amarelo no painel Problems e no `check_model` (`advice_for`, com os
   nós que mais pesam, `block_rows`, `--tiled` para o modelo do motor por tiles, o Check run a 1/4); acima de 100 %, a
   janela "Not much memory" (*Run anyway*, *Cancel*) antes de rodar. No lote: a estimativa x os workers (só no
   computador local), na linha de custo e na pergunta, com quantos workers cabem. A estimativa roda numa thread à
   parte (`Session::memory_poll`, uma por estado do modelo, do diário e dos dados, 500 ms depois da última edição), a UI
   só consulta (176 ns por quadro); a execução espera até 2 s por ela. Ficou: o `terraflux check` da CLI não mostra
   disco nem memória (só o `check_model`); o MCP/CLI não gravam `Entry::memory`; a memória de trabalho dentro de um
   operador não é vista.

## A documentação dentro do aplicativo, o Linkage Mapper aberto e a máscara de nuvens

Pedido de 2026-11-18, depois de uma análise do que ainda falta (a ferramenta foi julgada completa; ficaram três
lacunas de peso). Um commit por etapa, passando à seguinte sem perguntar; as ideias 1 e 2 andam ao mesmo tempo
(arquivos disjuntos: `crates/app` + `website` contra um módulo novo de `crates/ops`), a 2 num worktree com o seu
próprio target, as compilações em fila por um trinco (no máximo 4 núcleos ao todo). A 3 vem depois da 1, na árvore
principal. Sem release nem push sem pedido.

### 1. O aplicativo ligado à sua documentação

O site tem 99 tutoriais e a referência de cada operador, e o aplicativo não aponta para nenhum deles: o único link é
o dos releases (`app.rs`, About & updates). Não há menu Help, tela inicial nem caminho de um operador para a sua
página. Com 19 painéis e mais de 340 operadores, quem chega cai num canvas vazio.

- **1a. Menu Help e o módulo `docs`.** `crates/app/src/docs.rs`: a base do site (`https://sacridini.github.io/terraflux/`),
  `op_url(id)` com a mesma regra de `website/build.py` (`/operators/{assunto}/#op-{id com . e _ trocados por -}`, o
  assunto pelos assuntos de `builder.rs`, que o site lê por `subjects.json`), `tutorial_url`, `page(caminho)`; abre por
  `update::open_page`. Corrigir antes o `subject_of` de `build.py`: o nome exato do operador vence o prefixo (hoje
  `raster.*` cai todo em Data preparation e as entradas de nome completo de Raster analysis nunca casam), e um teste do
  app confere que `op_url` dá a página e a âncora que o site gera para todos os operadores. Menu **Help** depois de
  View: Quickstart, Tutorials, The application, Operator reference, Keyboard shortcuts (F1, que sai do View), *Report a
  problem...* (abre um issue novo do GitHub com versão, sistema e a última falha preenchidos, para a pessoa ler e
  enviar; nada sai sozinho) e *About Terraflux* (versão, licença, como citar pelo `CITATION.cff`, links). Também na
  paleta de comandos. **Feito** (Unreleased). `crates/app/src/docs.rs` (`op_url`, `page`, `tutorial_url`, `issue_url`,
  `open`, a janela About), o menu Help e as entradas da paleta; `build.py` com o nome exato vencendo o prefixo; os
  assuntos de `builder.rs` conferidos contra `website/data/subjects.json` por um teste, e `clumondo`, `molusce`,
  `futures`, `check` e `value` ganharam assunto (estavam em "Other", sem página no site).
- **1b. Cada operador leva à sua página.** No Node pane, ao lado de "more...", *Documentation* abre a página do
  operador; o menu de contexto de um operador na biblioteca e o de um nó no canvas ganham *Open its documentation*;
  shift+F1 (ação do keymap) abre a do nó selecionado. Os templates que têm tutorial (front matter `template:` em 45
  páginas) ganham *Read the tutorial* no menu Templates e na tela inicial: a tabela template → tutorial é gerada de
  `website/content` por `website/tools/tutorial_index.py` num arquivo do app (`include_str!`), e um teste falha quando
  ela fica velha.
  **Feito** (Unreleased), o Node pane, a biblioteca (menu e gaveta), o canvas, shift+F1 (`op_docs`, também no Help e
  na paleta) e o link *tutorial* do menu Templates; `tutorials.rs` + `tutorials.json` + `tutorial_index.py`, com
  os modelos publicados de cada tutorial já no índice (para a 1d). Fica para a 1c a tela inicial.
- **1c. A tela inicial.** Com o modelo em branco (nenhum nó, sem arquivo), o canvas mostra, em vez do vazio: *Start
  from your data* (abrir um raster ou vetores, o Wizard, soltar arquivos), *Start from a task* (os grupos dos
  templates, cada template com a sua frase e o seu tutorial), *Recent models* e *Learn* (Quickstart, Tutorials, a
  página do aplicativo). Some com o primeiro nó; volta com o modelo vazio de novo. Desenhada só quando visível (sem
  custo para o canvas com nós) e no tema escolhido; uma opção em Settings a desliga.
  **Feito** (Unreleased). `crates/app/src/start_page.rs` (`wanted`, `show`, devolve a `Act` que o `App::pane` executa pelo
  `command`, como os menus), a opção `start_page` (Settings > Model). Fora da ordem acima: *Recent models* e *Learn*
  vêm antes de *Start from a task* (o grid dos templates é longo e rola). Testes `ui the_start_page_*`.
- **1d. Os modelos dos tutoriais no aplicativo.** *Help > Tutorials with a model...*: uma janela com os tutoriais por
  categoria (as 21 de `site.yaml`), cada um com *Open the model* (o template, ou o `.toml` publicado em
  `/data/tutorials/`, baixado) e *Read* (a página). No site, os tutoriais com modelo ganham a linha "In Terraflux:
  Templates > ‹nome›" junto ao bloco do modelo. Sem protocolo `terraflux://` (registro no sistema em três plataformas:
  fica de fora).
  **Feito** (Unreleased). `crates/app/src/tutorial_models.rs` (a janela, o `plan` dos arquivos a baixar e o `download` com a
  busca injetável, sem rede nos testes), a pasta `tutorial models` dos dados do app; `tutorials.json` com `files` (todos os
  arquivos da pasta publicada); o site diz *In Terraflux: Templates > grupo > nome* (o grupo lido de `templates.rs` por
  `build.py`) ou *Help > Tutorials with a model > título*. Teste `ui the_tutorial_models_*`.

### 2. O Linkage Mapper aberto: corredores, pinch points e barreiras

O fluxo de corredores mais usado em planejamento (Linkage Pathways, Pinchpoint Mapper e Barrier Mapper, McRae e
Kavanagh 2011; McRae et al. 2012) depende do ArcGIS. O Terraflux tem as peças (custo, caminhos, Circuitscape), mas
nenhum operador com o fluxo. O terraflux-py também não tem: a referência é o algoritmo publicado e o código do
Linkage Mapper (Python sobre arcpy, que não roda aqui); os testes de paridade usam uma referência em Python escrita
com `skimage.graph.MCP_Geometric` (a mesma convenção do ArcGIS: passo de √2 na diagonal, média das resistências das
duas células, vezes o tamanho da célula) e o Circuitscape do terraflux-py. Módulo novo `crates/ops/src/linkage.rs`
(família `linkage.*`), distância de custo em Rust (Dijkstra em f64; a do gtb aceita só resistência u8).

- **2a. `linkage.pathways`.** Entradas: `cores` (mapa de ids, ou polígonos), `resistance`. Ajustes: adjacência
  (`cost`, pela alocação da distância de custo, ou `euclidean`), `max_cwd`, `max_euclidean`, `drop_crossing` (o par
  cujo caminho cruza outro núcleo sai), `width` (corte da distância normalizada), `nearest` (só os k vizinhos).
  Saídas: `corridors` (o mosaico, o mínimo das distâncias normalizadas CWD_A + CWD_B − LCD, cortado em `width`),
  `links` (tabela: from, to, euclidiana, LCD, comprimento do caminho, razão CWD/comprimento, mantido ou o porquê),
  `paths` (linhas), `cwd` (a distância de custo ao núcleo mais próximo). Dicas e conselhos (`tips.rs`, check).
  **Feito** (Unreleased). `linkage.pathways` em `crates/ops/src/linkage.rs`; `cores` só como mapa (polígonos: antes
  `vector.rasterize`); nodata ou resistência ≤ 0 é intransponível; `nearest` mantém a ligação se está entre as k
  menores de qualquer um dos dois núcleos; razão extra `unreachable`; a tabela só tem os pares candidatos. `linkage::cwd` e `edt` são
  públicas para 2b e 2c. Referência: `tools/make_linkage_reference.py` (MCP_Geometric), teste `linkage::`.
- **2b. `linkage.pinchpoints`.** Para cada ligação, o Circuitscape entre os dois núcleos só dentro do corredor cortado
  (ou all-to-one no mosaico todo); saídas `current` (o máximo ou a soma das ligações) e `links` (a resistência
  efetiva de cada uma).
  **Feito** (Unreleased). `linkage.pinchpoints` em `crates/ops/src/linkage.rs`, como o `s8_pinchpoints.py`: o
  corredor é CWD_A + CWD_B − LCD ≤ `width`; as ligações vão em paralelo pelo Circuitscape da biblioteca; `current` é o
  máximo das ligações (núcleos sem valor), o `all_to_one` usa todos os núcleos do mapa; `square` e a razão
  `cwd_to_resistance` como no LM; referência em `make_linkage_reference.py` (chave `pinch`). Faltou o `all_pairs` do LM.
- **2c. `linkage.barriers`.** Janelas circulares de raio `min`..`max` (passo `step`) restauradas: para cada célula e
  raio, a melhora = LCD − (mínimo de CWD_A no círculo + mínimo de CWD_B no círculo + o custo do círculo restaurado,
  `restored` por unidade), por unidade de distância restaurada (÷ 2R, como o Barrier Mapper); saídas `improvement`
  (o máximo sobre raios e ligações), `radius` (o raio que o deu), `percent` (÷ LCD).
  **Feito** (Unreleased). `linkage.barriers` em `crates/ops/src/linkage.rs`, como o `s6_barriers.py`: o mínimo é no anel
  (distância do centro acima de R − 1 e até R, em unidades do mapa), não no disco; `percent` é 100 × melhora / LCD;
  `restored` vale 1 por padrão (o LM fixa 1); `circles` (o máximo em disco de raio R) e `combine`, `cwd_threshold`
  como no LM; faltou o corte opcional pela resistência. Referência em `make_linkage_reference.py` (chave `barriers`).
- **2d. Template, tutorial, guia.** O template "Linkage Mapper: corridors, pinch points and barriers" (dados nlm.*),
  uma página de tutorial em `habitat-networks` ou `circuit-connectivity`, o guia `connectivity` do MCP e o README
  (tabela de campos: Linkage Mapper entre os métodos de).
  **Feito** (Unreleased). Template em `crates/app/src/templates.rs` (grupo Connectivity; cores = `raster.sieve` + `raster.label` de um
  habitat nlm), página `habitat-networks/linkage-mapper.md` (parte solta da série, ordem 70), guia `connectivity`, frases e método
  (`phrases.rs`, `methods.rs`) e a linha Connectivity do README.

### 3. A máscara de nuvens por pixel no `data.stac`

O `data.stac` filtra as cenas por `eo:cloud_cover` e compõe pela mediana, mas não mascara pixel a pixel: o SCL entra
só como mais uma camada (e em bilinear, se `classes` é falso). Na Amazônia e na Mata Atlântica, com nuvem
persistente, a mediana sem máscara contamina o mosaico. O terraflux-py (`download.stac`) também não mascara: a
ideia vai ao IDEAS.md dele, para manter a paridade.

- **3a. O operador.** Ajuste `mask`: `none` (padrão, o de hoje), `auto` (pela coleção), `scl` (Sentinel-2 L2A: o
  asset `scl`/`SCL`), `qa_pixel` (Landsat Coleção 2 Nível 2: os bits de QA_PIXEL), `fmask` (HLS). `mask_classes`:
  `cloud`, `shadow`, `cirrus`, `snow` (padrão: os três primeiros), e `mask_buffer` (células de dilatação). Para cada
  item, o asset de máscara do mesmo item é lido em `near` na grade, assinado como os outros, nunca vira camada; as
  células marcadas viram NaN antes do `combine`, que já as ignora. A máscara e as suas classes entram na chave do
  cache. Códigos conferidos na documentação dos produtos (SCL 3 sombra, 8 e 9 nuvem, 10 cirro, 11 neve; QA_PIXEL bits
  1 nuvem dilatada, 2 cirro, 3 nuvem, 4 sombra, 5 neve; Fmask bits 1 nuvem, 2 adjacente, 3 sombra, 4 neve) e no teste
  ao vivo (`#[ignore]`) de Earth Search e Planetary Computer. Testes com o `Fake` de páginas e GeoTIFFs locais.
  **Feito** (Unreleased). `crates/ops/src/stac.rs`: `compose` (a leitura, separada da busca), `mask_cells`, `dilate`, os códigos
  conferidos (QA_PIXEL no STAC do Planetary Computer, Fmask no guia do HLS v2.0 do LP DAAC; SCL e o preenchimento 255 do Fmask
  da memória do produto, ESA e USGS fora do ar) e dois testes ao vivo, ambos passando; os locais usam GeoTIFFs em disco.
- **3b. App e documentação.** Os presets de Sentinel-2 e Landsat do Data catalog ganham *Mask clouds per pixel*
  (marcado por padrão) e escrevem `mask = "auto"`; o tour (`getting-started/tour.md`), a referência e o guia
  `data-preparation` do MCP dizem como.
  **Feito** (Unreleased). `catalog.rs` (campo `mask` do preset, `stac_mask`/`stac_snow`/`stac_buffer`, crescer 1 célula por
  padrão), o exportador Python comenta a máscara (a biblioteca ainda não mascara), tour, canvas e guia `data-preparation`.

## "Use my data": qualquer template com os dados da pessoa, e o fim do Wizard

Pedido de 2026-11-18. Um commit por etapa, passando à seguinte sem perguntar; sem release nem push sem pedido.

Os 56 templates rodam com dados sintéticos (`nlm.*`); trocar pelos da pessoa é feito à mão, nó a nó (o template
"MSPA of your map" diz "choose its file in the node read, and in the reclassification the classes of its forest"), ou
pelo assistente (o prompt `adapt_template` do MCP). É onde o iniciante para: grades e CRS diferentes, sem dado, e
"qual classe do meu mapa é a floresta?". O Wizard (`wizard.rs`) confere os dados assim que escolhidos, mas só para
três modelos de mudança de uso, e escreve um modelo do zero. A ideia: levar essa conferência a todos os templates, e
depois aposentar o Wizard, que vira um caso do mesmo fluxo.

**A peça central: a tabela de classes.** Em vez de achar cada ajuste que depende das classes de um mapa (o
`foreground` do MSPA, as regras de uma reclassificação, as classes de uma legenda), um mapa de classes da pessoa
entra seguido de um `raster.reclassify` que traduz as classes dela para os códigos que o template espera ("as
suas classes 3 e 4 → floresta (1)"). Tudo o que vem depois continua como estava, e a tradução fica visível como um
nó com nota.

- **A. O que faz as vezes de dado.** Campo opcional `data` num nó (formato do modelo, `crates/graph`): este nó está no
  lugar de um dado da pessoa: `what` (a frase: "a land use map, the first date"), `kind` (`classes`, `values`,
  `mask`, `stack`), as classes esperadas com seus nomes (`classes = { 1 = "forest", ... }`) e, para valores, a faixa
  ou unidade esperada ("1 or more"). Lido e escrito em TOML e JSON, conferido pelo check, mostrado no Node pane, visto
  pelo MCP (`show_model`, o prompt `adapt_template`). Os templates ganham o campo nos seus nós de dados (os `nlm.*`
  que estão no lugar de um mapa, o `raster.calc` da segunda data dos de mudança de uso, os `stack.of` de fatores), e
  um teste exige que todo template com `nlm.*` tenha ao menos um. Os `nlm.*` sem o campo também são oferecidos, com a
  nota do nó como descrição.
  **Feito** (Unreleased). `Data`/`DataKind` em `crates/graph` (`classes` com o código como chave: chave nua no TOML, texto no JSON, de volta a inteiro),
  o check avisa, Node pane (*Stands for your data*, *Your data*, *Mark as data of yours...*), painel Changes, script Python, `edit_node`/`add_node`/`write_calc`
  e `show_model`/`adapt_template` do MCP, guia `record`; 116 nós de 49 templates TOML mais os três de mudança de uso em Rust (`LAND_CLASSES`), e o teste
  exige `data` de todo template com `nlm.*` (exceto os dois de paisagens neutras). Os `nlm.*` internos (ruído, base de outro mapa) ficaram sem marca.
- **B. O motor da troca** (`crates/app/src/my_data.rs`, puro): `sources(model)` (os nós com `data`, os `nlm.*`, os
  `raster.read` sem arquivo, as entradas `$x` do modelo sem valor); `plan(model, escolhas) -> Model` numa cópia: um
  `raster.read` (com a banda) no lugar de cada fonte; para classes, o `raster.reclassify` da tabela (classes não
  ligadas: sem dado, ou "outra" se o template tem uma); um `raster.match` para a grade de referência onde a grade
  difere (`near` para classes, `bilinear` para valores, como `fixes.rs` e `prepare.rs`); os `nlm.*` que ficarem
  postos na grade de referência (ou a troca recusada, dizendo qual falta); pilhas de um arquivo de bandas ou de
  vários; nota e `why` em cada nó novo. A conferência é a do Wizard (`load_inputs` + `lint`, numa thread) sobre o
  modelo planejado. Testes com os arquivos de teste (Sibuyan, Plum Island) em vários templates.
  **Feito** (Unreleased). `my_data.rs`: `sources`, `plan`, `check`, `classes_of`, `suggest`. O `raster.match` só entra onde o cabeçalho
  do arquivo mostra grade ou CRS diferentes da referência (`near` classes e máscaras, `bilinear` valores, `auto` o resto); o `data` vai
  no ÚLTIMO nó da cadeia (uma segunda troca o acha de novo; sem nó na cadeia, só a nota da entrada); o que só os nós trocados liam sai;
  `nlm.curds` (sem `like`) fica na sua grade e é dito nas notas. Os templates de classes quaisquer (métricas, tipos de paisagem, zonas) não nomeiam classes.
- **C. A janela.** *File > Use my data...*, a tela inicial e uma faixa no canvas quando o modelo aberto tem dados
  sintéticos ("This model runs on drawn data: Use my data..."). Passos: cada fonte (o que é, escolher o arquivo e a
  banda, a conferência ao lado, os achados com as correções); a tabela de classes de cada mapa de classes (as classes
  do template, com nome, contra as classes do arquivo, com nome da legenda e contagem); a grade de referência;
  revisão e *Apply*: um passo de desfazer, visto no painel Changes (`review::begin` antes de trocar o modelo), e uma
  execução.
  **Feito** (Unreleased). `my_data_view.rs`: a janela de três páginas (*Your data*, *Classes*, *Review*), a conferência de cada escolha numa
  thread (resultado de uma escolha antiga descartado pela chave), *Apply* = `review::begin` + um passo de desfazer + `read_data` + `run_when_ready`;
  aberta pelo File, a paleta, a tela inicial e a faixa do canvas (as fontes guardadas por revisão em `Session::my_data`, a faixa escondida por modelo).
- **D. Os modelos de mudança de uso.** Os templates Dinamica EGO, CLUE-S e PLUS com `data` nas suas fontes (duas
  datas de um arquivo de bandas ou de dois, a pilha de fatores, as datas), as mesmas conferências do Wizard (as
  classes de t0 e t1, a grade dos fatores) e, por último, os parâmetros do modelo (passos, incremento, elasticidade,
  ordem das classes...) num passo final com o formulário do painel Parameters. Paridade com o Wizard: o modelo que um
  e outro dão com os mesmos dados roda e dá as mesmas saídas.
  **Feito** (Unreleased). Duas datas de um arquivo de bandas ("The same file, band ...") ou de dois; página *Parameters* (os parâmetros expostos com o
  `params_view::control`, mais passos, incremento, expansão, isometria, método clue/ordered, ordem, elasticidade, datas, semente e "começa da 1ª ou
  2ª data"; as datas das bandas dão os primeiros valores); `lint` segue as portas que cada entrada alimenta (através de match/reclassify/legend/stack.of),
  não o nome; `Input::Map/Stack` ganham `data` (um só lugar: o último nó da cadeia, ou a entrada quando não há nó). Paridade: Dinamica, CLUE-S e ordered dão
  os mesmos mapas célula a célula que o Wizard com os mesmos arquivos, desde que a simulação comece da mesma data (o Wizard começa da 1ª, os templates da
  2ª: é a escolha "start") e, no CLUE-S, com a alocação do template (scale/max_iter) dada ao Wizard. Drivers em outra grade ficam `raster.match auto` (a média
  onde são mais grossos, como o botão Resample do Wizard), não `bilinear`.
- **E. O fim do Wizard.** O botão Wizard, a tela inicial e a paleta abrem o *Use my data* com uma primeira página
  "qual modelo de mudança de uso" (os textos de `Kind::about`); `wizard.rs` sai; testes, site (páginas do aplicativo,
  tutoriais que citam o Wizard), README ("a wizard that checks and fixes the data") e CHANGELOG atualizados. O
  `terraflux new dinamica` da CLI não usa o Wizard e continua.
  **Feito** (Unreleased). Antes, o CLUE-S com N datas: `Choice::dates` (todas as bandas de um arquivo, 3 ou mais, são as datas; a demanda lê a pilha do arquivo,
  `observed` vira `Input::Stack`, o nó que empilhava as duas datas fica como `ends` para o "começa da 1ª ou da última"; sem tabela de classes, pois `raster.reclassify`
  só lê mapa), oferta "All N bands as the dates" e caixa na janela; paridade célula a célula com o Wizard nas 3 datas de Plum Island (14 passos de 215.698 células, CLUE-S e
  ordered). Depois `wizard.rs` saiu: o botão da barra ("Use my data"), a tela inicial e a paleta abrem a janela na página *Which model* (`Kind`: Dinamica, CLUE-S, Ordered =
  template do CLUE-S com `method` ordered, PLUS), os testes de paridade montam a referência com os `presets`, os do ui são de Use my data, site/README/CHANGELOG sem o Wizard.

## Objetos: análise de imagem baseada em objetos (OBIA), no lugar do eCognition

Pedido de 2026-11-19: ter no Terraflux o que o Trimble eCognition faz (segmentar uma imagem em objetos, descrever cada
objeto, classificar por regras ou por amostras, em níveis), como operadores, e um Viewer que faça o mesmo ou mais; e
que seja **mais fácil** de montar e configurar que o eCognition, também para quem tem pouca experiência e está
começando. Por ora só o plano; quando for implementado, um commit por etapa, na ordem.
Acréscimo do mesmo dia: três ideias do InterIMAGE (PUC-Rio e INPE, sobre o GeoAIDA de Hannover; Costa et al. 2010)
que o eCognition não tem, nas etapas F (a árvore de classes), G (hipóteses por classe e a arbitragem) e H (várias
datas com a matriz de transição).

**Por que cabe no Terraflux.** O rule set do eCognition é uma árvore de processos: segmentar, calcular atributos,
classificar, fundir, de novo, em níveis. É um grafo de operadores, e quase tudo em volta já existe: o grafo com cache
e execução incremental (`crates/exec`), o motor por tiles (`crates/engine`) e o cluster (`crates/dist`), o Viewer em
GPU com composições RGB (`rgb.rs`), inspeção de célula e desenho (`draw.rs`: pontos, linhas, polígonos, vértices,
atributos), a tabela de atributos, os gráficos (`egui_plot`) e o histograma, o `trace_cell`, florestas aleatórias e
gradient boosting que já aprendem de tabelas (`ml.fit`/`ml.predict`/`ml.importance`), a acurácia de mapas, a
rotulagem e a vetorização (`raster.label`, `raster.sieve`, `vector.polygonize`, `vector.rasterize`, `raster.zonal`),
o analisador das calculadoras (`algebra.rs`, `vector_algebra.rs`), o `data.stac` com máscara de nuvens, o *Use my data*
(`my_data.rs`), o MCP e o assistente. Falta: um tipo de valor para objetos, os segmentadores, os atributos por objeto,
as regras fuzzy, os níveis, e o Viewer de objetos (contornos, inspeção do objeto, amostras por clique).
Duas vantagens que o eCognition não tem, de graça: mudar uma regra não refaz a segmentação (o cache), e o rule set é
um arquivo legível e versionado, com `note` e `why` em cada nó, rodável pela CLI, pelo cluster e pelo MCP.

**As patentes (conferido em 2026-11-19 no Google Patents; não é parecer jurídico).** Todas em nome da Definiens AG
(a parte de geociências, o eCognition, foi para a Trimble em 2010; o registro não mostra cessão). Expiradas: a da
segmentação em várias resoluções com hierarquia de objetos semânticos (US7437004, Baatz, Schäpe e Schmidt, prioridade
1999-12-14, expirou em 2021-05-30; a família europeia EP1238364 também), a dos "darwinian objects" (US6738513,
expirou em 2018-10-26), a da rede semântica de cognição (US7146380, expirou em 2025-01-04) e, pela data ajustada, a da
"cognition integrator and language" (US7873223, 2026-10-03; o Google ainda diz "Active", conferir). **Ativas:**
US7801361 (até 2029-01-20: filtrar os pixels ligados aos objetos de uma classe para uma nova camada de imagem, formar
dela uma segunda rede de objetos, medir e mostrar) e US8319793 (até 2031-09-28: "imprimir" os objetos de uma rede em
outra, formando uma terceira, e medir). As reivindicações são gerais, não só de imagens médicas. Consequências para o
plano: a segmentação multirresolução (Baatz & Schäpe 2000) e os níveis feitos por fusão de baixo para cima estão
livres; as duas operações das patentes ativas (re-segmentar uma camada filtrada por objetos dentro do mesmo fluxo de
objetos; cruzar duas segmentações num terceiro nível) ficam para depois de 2029/2031, ou só com uma análise de
liberdade de operação. A arbitragem de hipóteses de classes diferentes (etapa G) fica perto da US8319793: é
feita célula a célula sobre mapas de pertinência, e o GeoAIDA (Bückner et al. 2001) e o InterIMAGE (Oliveira et al.
2008, Costa et al. 2010) a publicaram antes do depósito (2009-04-17), mas é a etapa que pede a análise jurídica antes
de um lançamento. Patente é territorial: não conferi a família no INPI nem as patentes próprias da Trimble (a
busca por titular do Google Patents não abre sem navegador).

**Princípios.**
1. **Toda ação do Viewer vira nó**, como o *Filter the selection*: segmentar com a escala escolhida no controle
   deslizante, rotular amostras por clique, transformar um limiar visto no mapa numa regra. O resultado é sempre um
   modelo que roda de novo sozinho, com um passo de desfazer, visto no painel Changes.
2. **Um caminho curto e o grafo inteiro.** O iniciante abre uma imagem, escolhe as classes, clica em alguns objetos
   de cada uma e tem um mapa: um template (ou o Viewer) monta os nós por trás. O especialista abre esses mesmos nós e
   troca o segmentador, os atributos, as regras e os níveis. Não há um "modo fácil" à parte: é o mesmo modelo.
3. **Padrões que funcionam sem ajuste.** A escala estimada da imagem (ESP), os pesos das bandas iguais, um conjunto
   básico de atributos, floresta aleatória como classificador padrão, e o check avisando o que costuma dar errado.
4. **Explicar cada objeto.** Sob o cursor e no Trace: os atributos do objeto, o grau de pertinência a cada classe, a
   regra ou o voto das árvores que decidiu, e os vizinhos e o objeto-pai. É onde se passa do eCognition.
5. **Sem dependência nova de build pesada.** Rust; o ONNX Runtime (para o SAM, na última etapa) carregado em tempo de
   execução como o GDAL (`libloading`), e o resto funcionando sem ele.

**O tipo novo.** `Type::Objects` / `Value::Objects` (`crates/graph`, `crates/ops/src/value.rs`): a grade, os rótulos
`Arc<Vec<u32>>` (não f64: 4 bytes por célula), o número de objetos, a caixa e a contagem de células de cada um, o grafo
de adjacência (vizinho e comprimento da borda comum), o nível de cima (`Option<Arc<Objects>>`, cada objeto com o seu
pai, um nível sempre contido no de cima), a tabela de atributos (`Table`, uma linha por objeto) e as classes com os
graus de pertinência. Mostrado no Viewer, lido pelo `trace_cell`, gravado no cache (bincode + lz4, como o motor por
tiles), visto pelo MCP (`show_model`, `inspect_file`).

**As etapas.**

- **A. A base.** O tipo `Objects`; `segment.from_map` (um mapa de classes ou de rótulos qualquer vira objetos: a
  porta de entrada de qualquer segmentador externo, inclusive SAM rodado fora), `segment.from_vector` (polígonos viram
  objetos, a tabela vira atributos), `segment.chessboard` e `segment.quadtree` (os mais simples do eCognition);
  `objects.to_map` (a classe, um atributo ou o id como mapa), `objects.to_vector` (polígonos com os atributos e a
  classe, suavização opcional, gravados pelo `vector.write`). O Viewer mostra uma camada de objetos como contornos sobre
  a imagem (cor e espessura na linha da camada). E `nlm.image`: uma imagem multiespectral sintética a partir de um mapa
  de classes (`nlm.*`) e de assinaturas espectrais (água, floresta, campo, cultivo, cidade; ruído, textura e borda
  borrada), com a verdade conhecida: o dado dos templates e dos testes de acurácia. Testes: cada objeto conexo, os
  rótulos contíguos, ida e volta mapa → objetos → mapa e vetor → objetos → vetor.
  **Feito** (Unreleased). `crates/ops/src/objects/` (`mod.rs` o tipo e `from_regions` por union-find, `store.rs` o
  binário gzip dos nós congelados, `segment.rs`, `convert.rs`); `nlm.image` em `nlm.rs`; o Viewer em
  `crates/app/src/objects_view.rs` (contornos numa imagem do tamanho da tela, refeita só quando a vista muda) e
  `map_view.rs`. O Chaikin do `to_vector` encolhe cantos e suaviza cada objeto à parte (vizinhos podem não casar).
- **B. Os segmentadores.** `segment.multiresolution` (Baatz & Schäpe 2000: escala, peso de cor e forma, compacidade e
  suavidade, peso das bandas; a partir dos pixels ou de um nível existente, para fazer um nível de cima; determinístico
  com a mesma semente; em paralelo pela abordagem de Happ et al. 2010, PUC-Rio, por blocos com a fusão entre blocos
  feita à parte). `segment.slic` e `segment.snic` (superpixels, Achanta et al. 2012 e 2017), `segment.felzenszwalb`
  (2004), `segment.watershed` (gradiente, marcadores opcionais), `segment.meanshift` (depois, o LSMS do OTB como
  referência). `segment.scale` (o ESP de Drăguţ et al. 2010/2014: a variância local por escala e o seu ganho, as
  escalas sugeridas, uma tabela e um gráfico): é o que dá a escala padrão do `segment.multiresolution` quando ela não é
  dada. Testes: invariantes (a heterogeneidade de cada fusão abaixo do limiar, a imagem homogênea vira poucos objetos,
  escala maior dá menos objetos, determinismo entre 1 e 8 threads), e a qualidade na `nlm.image` (a sobre e a
  subsegmentação contra a verdade). Benchmarks com no máximo 8 threads.
  **Feito** (Unreleased). `objects/multiresolution.rs` (grafo de regiões com n, Σx, Σx² por banda, regiões "quietas"
  fora das visitas; uma thread por ora: o paralelo por blocos fica na etapa K; 1 Mcélula × 4 bandas em 2,7 s),
  `superpixels.rs` (SLIC, SNIC, Felzenszwalb; bandas padronizadas por padrão, a `compactness` do SLIC/SNIC padrão 1
  com a cor pela média por banda), `watershed.rs` (Sobel, o máximo das bandas, h-minima por persistência, compacto de
  Neubert & Protzel), `meanshift.rs` (filtro plano em rayon pelo teto central, agrupamento pela média da região),
  `esp.rs` (ESP2 hierárquico numa janela central de 512², 0,8 s; o primeiro pico do ROC médio é a escala padrão do
  multiresolution, dita pela saída `scale` e por um aviso). Gráfico *ESP* em `charts.rs` pela forma da tabela.
- **C. Os atributos.** `objects.features`: lê os objetos e uma ou mais imagens ou mapas (qualquer raster como camada:
  MDE, nDSM, um mapa de classes anterior, a imagem de outra data) e dá os objetos com a tabela, e a tabela à parte.
  Grupos: **espectrais** (média, desvio, mínimo, máximo, mediana por banda, brilho, diferença máxima, razão; os índices
  ditos pelos nomes das bandas, NDVI e NDWI); **forma** (área, perímetro, compacidade, índice de forma, comprimento e
  largura pelo retângulo mínimo, assimetria, direção principal, ajuste à elipse e ao retângulo, índice de borda);
  **textura** (GLCM de Haralick por objeto: homogeneidade, contraste, entropia, correlação, segundo momento angular);
  **contexto** (diferença média aos vizinhos, borda relativa a vizinhos de uma classe, distância a uma classe, o
  atributo do pai, a contagem e a área relativa dos filhos de uma classe). Um conjunto `basic` (o padrão: médias,
  desvio, NDVI quando há vermelho e infravermelho, área, compacidade, homogeneidade) e `all`. Uma passada pelos pixels,
  em paralelo por faixas de linhas. Testes: cada atributo contra a conta à mão num objeto pequeno; as médias contra
  `raster.zonal` com os rótulos.
  **Feito** (Unreleased). `objects/features.rs`: os grupos em `GROUPS` (spectral, shape, texture, context), em paralelo
  por objeto (o mesmo resultado com qualquer número de threads); retângulo mínimo por casco convexo e rotating calipers;
  a GLCM quantizada entre os percentis 1 e 99 da imagem; `dist_<classe>` por uma grade de baldes; `below` dá os filhos.
  `all` em 10 000 objetos de 1000×1000×4: 112 ms (83 colunas). A mais: `ndmi` quando há SWIR.
- **D. Classificar.** Três caminhos, que se combinam:
  - **Por amostras com ML.** `objects.samples`: as amostras de pontos ou polígonos (`vector.read`, um desenho do
    `draw.rs`, ou os cliques do Viewer, etapa I) viram a tabela de treino (o objeto sob cada amostra, a classe, os
    atributos), com aviso de objeto com duas classes. Depois `ml.fit` e `ml.predict` como já são, sobre a tabela; e
    `objects.classify` lendo o `MlModel` dá as classes e as probabilidades de cada uma (a pertinência).
  - **Por regras fuzzy.** `objects.calc`, a calculadora dos objetos (o analisador de `vector_algebra.rs` com funções
    de objetos): `nb.mean(x)`, `nb.count(cls == "water")`, `border_to("water")`, `distance_to("road")`,
    `parent.x`, `children.count(cls == "tree")`, `children.area(cls == "tree")`; e as funções de pertinência
    `above(x, a, b)`, `below(x, a, b)`, `between(x, a, b, c, d)`, `about(x, m, w)`, com `and` = mínimo, `or` = máximo,
    `not` = 1 - x quando os termos são graus. Escrita com o `write_calc` do MCP. `objects.rules`: uma regra por classe,
    uma hierarquia de classes com herança (a filha herda a regra da mãe, e soma a sua), o limiar mínimo de pertinência
    (abaixo dele, "sem classe") e a classe dada pela maior pertinência, como o eCognition.
  - **Por vizinho mais próximo.** `objects.nearest` (o "Standard Nearest Neighbor" do eCognition: a pertinência pela
    distância no espaço dos atributos às amostras de cada classe), para quem conhece esse fluxo.
  - **A acurácia por objetos.** `objects.accuracy`: amostras de validação (separadas das de treino, ou um sorteio
    estratificado de objetos para rotular), a matriz de confusão, a acurácia global e por classe e as áreas ajustadas,
    pelos operadores de acurácia que já existem sobre o mapa das classes.
  Testes: na `nlm.image`, acurácia acima de um limite com poucas amostras por classe; as regras fuzzy contra a conta à
  mão; a hierarquia de classes.
  **Feito em parte** (Unreleased): os três caminhos. `objects/classify.rs` (`objects.samples`: a tabela de treino sem
  o id, para o `ml.fit` só com `column = "class"`; `objects.classify`), `calc.rs` (`objects.calc`: o analisador em modo
  objetos, `parse_script_objects`, as chamadas de objetos viram colunas ocultas antes do script; `and`/`or`/`not` e as
  funções de pertinência reescritos na árvore; `cls` é "unclassified" sem classe), `rules.rs` (`objects.rules`, a
  hierarquia lida por `tree`; `objects.nearest`, as cores como espaço padrão). Na `nlm.image`: 95–100% por amostras,
  99–100% por regras e por vizinho mais próximo. A acurácia: `objects/accuracy.rs` (`objects.accuracy`, por objetos
  e ajustada por área com `accuracy::olofsson`, as células de cada classe como estratos; `objects.sample`, a amostra
  estratificada de objetos a rotular, pontos no objeto perto do centroide). **Feito** (Unreleased).
- **E. Os níveis e as operações sobre objetos.** Um nível de cima feito do de baixo (`segment.multiresolution` com
  `from`, ou `objects.merge` por classe: os vizinhos da mesma classe viram um objeto), `objects.merge` por uma condição
  (`objects.calc` sobre o par de vizinhos), `objects.grow` (crescer objetos de uma classe para os vizinhos que atendem a
  uma condição, até não mudar mais), `objects.remove_small` (o `raster.sieve` dos objetos: o pequeno vai para o vizinho
  mais parecido ou de maior borda), `objects.reclassify` (renomear e juntar classes). Os laços "até não mudar mais" pelo
  `control.repeat`, com a parada quando o número de objetos não muda. Os níveis só por fusão: nada de imprimir os objetos de
  uma segmentação noutra para formar um terceiro nível nem de re-segmentar uma camada filtrada pelos objetos (as
  patentes ativas); segmentações diferentes só se encontram pela arbitragem célula a célula da etapa G.
  **Feito** (Unreleased): `objects/levels.rs` (`regroup`, o núcleo comum: uma linha da tabela por objeto que ficou
  sozinho, a média ponderada por células nos fundidos; objetos com pai só se fundem com os do mesmo pai). Cada um
  dá `count` e `done` (o número de objetos não mudou), para o `until` do `control.repeat`. A condição do par lê
  `a.x`, `b.x` e `border`.
- **F. A árvore de classes (do InterIMAGE).** O que existe na cena, dito primeiro: Cena → Urbano (Prédios, Ruas),
  Vegetação (Árvores, Grama), Água. Cada nó da árvore é uma classe (nome, cor, a classe-mãe) e pode ter o seu ramo do
  modelo, não só as folhas: o ramo de um filho roda só dentro da região achada para o pai (a máscara do pai, como o
  "ShapeFile Intersection" do InterIMAGE), e "ache a cidade, depois os prédios dentro dela" é o próprio grafo. No
  modelo: a árvore é a hierarquia de classes do `objects.rules` (etapa D), e cada ramo um `control.group` com
  `@region` (a região do pai) e a classe que procura. Uma vista *Classes* no canvas mostra o modelo pela árvore (um
  cartão por classe com o seu método, a regra, as amostras e a área achada), ao lado da vista do grafo; *Add a
  class...* cria o nó da árvore e o seu grupo, com receitas prontas por classe ("water by NDWI", "vegetation by NDVI",
  "buildings by height", "by samples"). Para quem está começando é o ponto de partida mais natural ("o que há na minha
  imagem?") em vez de "qual a escala?". Testes: o filho não acha nada fora do pai; a árvore lida e escrita com o
  modelo; uma receita aplicada a um nó roda.
  **Feito** (Unreleased): `crates/app/src/class_tree.rs` e `class_view.rs`. Um ramo é um `control.group` com os
  `params` `class`, `parent_class` e `colour` (o `control.group` os declara), que lê `@region` e dá `membership`;
  os irmãos num `stack.of` e um `objects.resolve`, a região de cada mãe um `raster.calc` do `class` dele. O botão
  *Classes* do painel Model mostra os cartões; *Add a class...* com as receitas NDWI, NDVI, altura e amostras, num
  passo de desfazer. Depois: a receita *made of its daughters* (a mãe como a união fuzzy, o máximo, das filhas, que
  leem a região da avó: de baixo para cima, como no InterIMAGE) e os prédios por altura e forma (`rectangular_fit`).
- **G. Hipóteses por classe e a arbitragem (do InterIMAGE).** Cada classe achada pelo seu próprio método (um limiar de
  NDWI para a água, a segmentação multirresolução com regras de forma para os telhados, o nDSM para as árvores, o SAM),
  sem que um método tenha de servir para todas. Cada ramo da etapa F dá um mapa de pertinência da sua classe (0 fora
  das suas hipóteses; dentro, o grau da sua regra fuzzy ou a probabilidade do `ml.predict`). As hipóteses de classes
  irmãs se sobrepõem, porque os ramos não se veem. `objects.resolve` lê os mapas de pertinência das classes irmãs (uma
  pilha, uma camada por classe) e decide célula a célula pela maior pertinência: o limiar mínimo ("sem classe" abaixo
  dele), o empate pela ordem das classes, e a junção depois, `connected` (um objeto por área conexa, o padrão),
  `global` (um objeto por classe, mesmo não contíguo) ou `none`. Uma hipótese em parte sobreposta não some inteira:
  perde só a parte em que outra ganhou, como no InterIMAGE. Saídas: os objetos com a classe e a pertinência vencedora,
  o mapa da segunda maior pertinência (a dúvida, que alimenta o "onde rotular a seguir" da etapa I) e a tabela dos
  conflitos (quanta área de cada classe perdeu para cada outra). Sobe a árvore: a região de Vegetação é a união das
  vencedoras de Árvores e Grama. No Viewer, as hipóteses de cada classe como camadas e o conflito da área sob o cursor
  ("Trees 0.82 venceu Grass 0.64"). A etapa fica separável das outras, para não entrar num lançamento antes da análise
  da patente (a nota acima). Testes: a sobreposição parcial, os três modos de junção, a arbitragem contra a conta à
  mão num mapa pequeno.
  **Feito** (Unreleased), o operador: `objects/resolve.rs` (`objects.resolve`, as saídas `class`, `membership`,
  `doubt` e `conflicts`; `union_region` para a etapa F). O Viewer veio com a etapa I (`hypotheses.rs`). Saiu
  na 1.56.0 sem a revisão jurídica: é o item 1 do bloco seguinte, abaixo.
- **H. Várias datas com a matriz de transição (do InterIMAGE).** As datas como ramos da árvore (os "nós temporais" do
  InterIMAGE), com duas estratégias. **Sequencial:** as classes validadas da data 1 dão as hipóteses iniciais da data 2
  (os objetos da 1 como partida da segmentação da 2, e a sua classe como atributo). **Síncrona:** as datas classificadas
  em paralelo, e depois `objects.temporal` ajusta as pertinências de cada data pela possibilidade de cada transição
  entre classes, uma matriz fuzzy (Feitosa et al. 2009, cadeias de Markov fuzzy: a composição máx-mín das pertinências
  da data 1 com a matriz limita as da data 2), antes da arbitragem da etapa G. Transições impossíveis (cidade →
  floresta primária em um ano) perdem força; na tese de Costa, a acurácia média por classe foi de cerca de 55% (uma
  data por vez) para entre 65% e 94%. A matriz: escrita à mão numa tabela (uma linha por classe de origem, de 0 a 1),
  tirada do `change.transitions` de mapas anteriores (as taxas viram possibilidades), ou estimada das amostras das
  duas datas. É a ponte com o resto do Terraflux: o mapa de mudança por objetos entra direto nos modelos de mudança de
  uso como as datas observadas. Testes: uma transição de possibilidade 0 nunca acontece; com a matriz toda 1, igual a
  classificar cada data sozinha; o ganho de acurácia numa `nlm.image` de duas datas com ruído.
  **Feito** (Unreleased): `objects/temporal.rs` (`objects.temporal`, a pertinência anterior de cada objeto como a
  média ponderada por célula da outra data, a composição max-min; `objects.transition_matrix`, a matriz das amostras
  das duas datas ou do `change.transitions`). Com a matriz toda 1 o resultado é `min(μ, max μ_antes)`: igual a
  classificar só a data se o antes for nítido.
- **I. O Viewer dos objetos.** Onde o eCognition prende quem o usa, e onde dá para ir além.
  1. **A camada de objetos:** contornos de um nível sobre a imagem, preenchimento pela classe (a legenda das classes,
     a transparência), a troca de nível, o objeto sob o cursor destacado. Um clique seleciona o objeto (e os seus
     vizinhos, pai e filhos com teclas).
  2. **O inspetor do objeto:** os atributos, a pertinência a cada classe em barras, a regra ou as árvores que
     decidiram, o pai e os filhos; o mesmo pelo `trace_cell` do MCP.
  3. **A segmentação ao vivo:** o controle deslizante da escala (e de forma e compacidade) re-segmenta só a janela
     visível (como o `calc_view::window_in`), numa thread, em menos de um segundo numa janela de tela; *Apply* grava o
     nó `segment.multiresolution` com os valores. A curva do ESP ao lado, com as escalas sugeridas marcadas.
  4. **Amostras por clique:** a paleta das classes (nome e cor; acrescentar uma classe ali mesmo); um clique dá a
     classe ao objeto, arrastar pinta vários, o botão direito tira. Grava um nó `objects.samples` com a tabela das
     amostras no próprio modelo (como os atributos do desenho), ou num GeoPackage ao lado quando são muitas. As
     amostras sobrevivem a uma nova segmentação: guardadas como pontos no centro de cada objeto clicado.
  5. **O espaço dos atributos:** o diagrama de dispersão de dois atributos com as amostras coloridas pela classe, o
     histograma de um atributo por classe, a separabilidade (Jeffries-Matusita) de cada par de classes por atributo, e a
     sugestão dos atributos que mais separam (como o "Feature Space Optimization", mas mostrado). Um ponto no gráfico
     acende o objeto no mapa, e vice-versa.
  6. **Do limiar à regra:** escolher um atributo, arrastar a faixa no histograma e ver os objetos acesos no mapa; *Make a
     rule* cria (ou acrescenta à) regra da classe no `objects.rules`, com as bordas como pertinência fuzzy.
  7. **Onde rotular a seguir** (aprendizado ativo): depois de classificar, os objetos de maior incerteza (a menor
     diferença entre as duas maiores probabilidades) numa lista, um a um no mapa, para clicar a classe; e os erros da
     validação (a amostra e a classe dada) numa lista que leva a cada um.
  8. Comparar: a cortina, o lado a lado e a diferença que o Viewer já tem, entre níveis, entre classificações e entre
     duas datas.
  **Feito em parte** (Unreleased): 1, 2 e 4 (`crates/app/src/objects_inspect.rs`: a seleção ligada à tabela, N/P/C
  e Esc, o seletor de nível; o inspetor com as barras de pertinência e o que decidiu; o modo *Samples*, cada amostra um
  ponto na célula interior perto do centroide, gravada como `table.of` → `vector.from_table` → `objects.samples` →
  `ml.fit` → `objects.classify` num passo de desfazer, num GeoPackage acima de 500). Depois, o resto: 3
  (`live_segment.rs`: os controles de escala, forma e compacidade re-segmentam só a janela visível numa thread, 0,3 a
  0,8 s numa janela de 800 × 600, com a curva do ESP e as escalas sugeridas; `objects::segment_window`), 5 a 7
  (`feature_space.rs`: a dispersão, o histograma por classe, a separabilidade de Jeffries-Matusita e os atributos que
  mais separam; a faixa do histograma vira regra no `objects.rules` com as bordas fuzzy; os objetos de menor margem e os
  erros da validação, cada um levando ao objeto) e 8 (a cortina, o lado a lado e a diferença entre dois `Objects`).
  A etapa G no Viewer: `hypotheses.rs`, a pertinência de cada classe como camada e, sob o cursor, o vencedor, a dúvida e
  o conflito. **Feito** (Unreleased).
- **J. Para quem está começando.** O que faz do Terraflux mais fácil que o eCognition:
  1. **Templates** (com a `nlm.image`, rodam como estão; com `data` em cada fonte para o *Use my data*): "Classify an
     image by clicking samples" (o principal: segmentar, amostras, floresta aleatória, acurácia, polígonos), "Classify an
     image by rules" (água por NDWI, vegetação por NDVI, o resto por contexto), "Land cover change by objects" (duas
     datas, os atributos das duas, a mudança por objeto), "Segment an image into polygons" (só segmentar e exportar),
     "Each class its own way" (a árvore de classes da etapa F, uma receita por classe e a arbitragem da etapa G), e
     "Trees and buildings from a height model" (nDSM + imagem, regras de altura e forma).
  2. **O *Use my data* para imagens:** `DataKind::Image`, com os papéis das bandas (azul, verde, vermelho,
     infravermelho, SWIR) tirados dos nomes (Sentinel-2, Landsat 8/9, como os atalhos do `rgb.rs`) ou escolhidos numa
     página *Bands*; a escala da reflectância (0–1 ou 0–10 000) detectada e normalizada; e as classes da pessoa, com
     nome e cor, numa página *Classes*. E sem imagem própria: "desenhe a área e baixe" uma cena Sentinel-2 sem nuvens
     pelo `data.stac`.
  3. **Padrões e avisos** (`tips.rs`, `advice.rs`, o check): a escala do ESP quando não dada; aviso de menos de 10
     amostras numa classe, de classe sem amostra, de classes muito desbalanceadas, de objetos muito menores ou maiores
     que as amostras desenhadas, de imagem sem CRS, de bandas em escalas diferentes, de dois atributos quase iguais; e o
     que fazer em cada caso.
  4. **O guia `objects`** do MCP (o método passo a passo, os operadores, os templates) e o prompt "classify this image
     into water, forest and city": o assistente monta o modelo, abre no Viewer e pede as amostras.
  5. **Tutoriais** no site e no Menu Ajuda: "o primeiro mapa por objetos em 10 minutos", e um para quem vem do
     eCognition (o que é cada coisa aqui: process tree → grafo, level → `Objects` com pai, class hierarchy →
     `objects.rules`, feature view → espaço dos atributos, customized feature → `objects.calc`), e o mesmo para quem vem do InterIMAGE (rede semântica → árvore de
     classes, operador top-down → o ramo de uma classe, regra bottom-up → `objects.resolve`, nó temporal →
     `objects.temporal`).
  **Feito** (Unreleased): 1 (seis templates no grupo *Objects (OBIA)*, o de amostras com 94% de acurácia contra a
  verdade), 2 (`DataKind::Image`; a página *Bands* com os papéis das bandas de Sentinel-2 e Landsat e a escala
  0–10 000 ou Landsat C2 detectada, num `stack.rename` que renomeia, descarta e escala; as classes do `objects.rules`
  editáveis na página *Classes*), 3 (`objects/warnings.rs` e o `advice.rs`) e 4 (o guia `objects`). Falta: "desenhe a
  área e baixe" e a pasta de bandas soltas vieram depois: uma pasta ou arquivos de uma banda empilhados na ordem das
  bandas (`raster.match` para as de 20 m), e *No image of your own?*, uma área e datas dão um `data.stac` do Sentinel-2
  L2A sem nuvens (a mediana, `mask = auto`), com o desconto de 0,1 da baseline 04.00. E 5: três tutoriais no site e na
  Ajuda ("Your first map by objects in 10 minutes", "Coming from eCognition", "Coming from InterIMAGE"). **Feito**
  (Unreleased).
- **K. Imagens grandes.** Segmentar por tiles do `crates/engine` com a costura dos objetos na borda (o método de Happ et
  al. ou o do LSMS do OTB: segmentar cada tile com margem e decidir os objetos que cruzam a borda numa segunda passada),
  os atributos por faixas sem a imagem inteira em memória, os rótulos em disco quando passam do limite, e o cluster
  (`crates/dist`) por tiles. Meta: uma cena Sentinel-2 inteira (10 980 × 10 980, 10 bandas) em um computador comum, e
  uma ortofoto de 50 cm de um município.
  **Feito em parte** (Unreleased): `objects/tiles.rs`, `segment.multiresolution` com `tile` e `margin` (automático
  acima de 4 M células): cada tile com a sua margem em paralelo, só o núcleo guardado, e uma segunda fusão dos pedaços
  sobre a imagem inteira junta os objetos cortados pela borda. 4000 × 4000 × 4: 74 s → 30 s com 3 threads. (O "0,98 das
  bordas iguais" era a fração de células com o mesmo estado de borda/não borda, que numa imagem de objetos grandes é
  quase 1 por qualquer motivo: não media a costura. A medida certa, a fração dos pares de células vizinhas em objetos
  diferentes, mostrou a costura: 71 % dos pares sobre a borda de um tile contra 10 % nas outras linhas num recorte
  Sentinel-2 de 580 × 580, e 22 % no crescimento de regiões; na imagem sintética de `objects_tiles.rs`, 0,438 contra
  0,268. A causa: cada janela segmentava a sua sobreposição sozinha e a segunda fusão só juntava pedaços cujo custo de
  fusão ficasse abaixo de scale², mas o custo de dois pedaços grandes cresce com o tamanho, por mais parecidos que
  sejam: o objeto cortado ficava cortado, e um `margin` maior não ajudava.) **Corrigido** (Unreleased): os pedaços que
  tocam uma borda de tile voltam a células soltas antes da segunda fusão (`dissolve_at_borders`, `tiles.rs`), que as
  faz crescer através da borda como em qualquer outro lugar; são cerca de 5 % das células. Agora a fração cortada na
  borda é a das outras linhas (multiresolution 0,273 contra 0,267, 2000 × 2000 × 4, tiles de 512), e o número de
  objetos e o F dos contornos contra a imagem inteira ficam dentro do que outra semente dá
  (`tiled_multiresolution_has_no_seam`, `tiled_region_growing_has_no_seam`). É a ideia de Lassalle et al. 2015 (IEEE
  TGRS, "A scalable tile-based framework for region-merging segmentation"): o que toca a borda é instável e é refeito
  junto com o vizinho; aqui refeito a partir das células, com o critério da imagem inteira. Custo: a segunda fusão
  (uma thread) tem ~5 % de regiões a mais, +19 % no tempo total com 4 threads em 2000 × 2000 (3,7 → 4,4 s), e o pico de
  memória de uma thread sobe de 353 para 480 MB.
  Os rasters em disco por janelas (`objects/big.rs`): os tiles leem só a sua janela, a costura e a tabela releem por
  faixas, e `objects.features` por faixas (a mediana e o GLCM exatos); a imagem nunca inteira na memória. 4000 × 4000
  × 4 do disco: 66 s e 1,2 GB de pico. Um limite de memória (metade da física) recusa ler inteira o que não cabe. O
  cluster: `TileCores` e `TileJob` no ops, o `Ctx` e o `Engine` com um `tile_runner`, e o `Remote` do `crates/dist` com o
  trabalho `segment` (os tiles nos workers, a costura no coordenador; os mesmos rótulos que aqui); o `--coordinator` da
  CLI sem `--tiled` o liga. **Feito** (Unreleased).
- **L. Aprendizado profundo (opcional).** `segment.sam` (Segment Anything por ONNX, o ONNX Runtime carregado em tempo de
  execução; os pesos baixados uma vez para o cache; sem ele, o operador diz como instalar) e `objects.embed` (a média
  por objeto das incorporações de um modelo de base, como Clay ou Prithvi, como atributos para o `ml.fit`). O resto do
  módulo nunca depende disso.
  **Feito** (Unreleased): `objects/onnx.rs` (o ONNX Runtime 1.17 carregado por `libloading`, `ORT_DYLIB_PATH` ou ao
  lado do executável; os índices da tabela `OrtApi` conferidos com o header do 1.17) e `objects/deep.rs`
  (`segment.sam`, janelas de 1024, a máscara de melhor IoU de cada ponto; `objects.embed`, as colunas `emb_*`). Sem
  teste com um runtime de verdade (o item 2 do bloco seguinte).

**Decisões tomadas** (2026-11-18): os nomes `segment.*` e `objects.*`; o `python_script` avisa que esses nós não
têm equivalente no terraflux-py (até o porte do bloco seguinte); as amostras no modelo (num GeoPackage ao lado acima
de 500); LiDAR só por rasters feitos fora; a árvore de classes como vista sobre `control.group` + `@region` (nada muda
no formato); a matriz de transição fuzzy das amostras das duas datas (o algoritmo genético de Feitosa et al. não foi
feito).

**Lançado na 1.56.0** (2026-11-19), as etapas A a L.

### O que ficou do OBIA, e o bloco seguinte (pedido de 2026-11-19)

Na ordem em que são feitos, um commit por fase, de uma para a outra sem parar:

1. **A análise de liberdade de operação do `objects.resolve`** (a etapa G) diante da US8319793 e da US7801361 da
   Definiens/Trimble e das outras patentes delas em vigor: as reivindicações elemento a elemento, a família no EP e no
   INPI, a arte anterior (GeoAIDA 2001, InterIMAGE 2008/2010) e o que o operador não pode fazer. É uma leitura
   técnica; a opinião formal fica com um advogado de patentes.
   **Feito** (`docs/patents-objects-resolve.md`): nenhuma reivindicação independente da US8319793 se lê no
   `objects.resolve` (faltam as duas redes de objetos e a impressão de uma na outra), e pelo *terminal disclaimer* ela
   deve acabar com a US7801361, por volta de 2029-01-20, não em 2031. As duas só valem nos EUA (os parentes europeus
   foram recusados ou expiraram; nada no INPI). O risco está num fluxo depois do `resolve` (editar o mapa de classes
   nos objetos de uma classe, segmentar de novo e reclassificar: US7801361, reivindicações 25 e 28), que o Terraflux
   não oferece, e no classificador por pixel aprendido de objetos (US10445557, até 2035). As regras de projeto estão no
   documento; um advogado ainda deve confirmar o disclaimer, o prazo, o titular e o INPI.
2. **O ONNX Runtime de verdade**: `segment.sam` e `objects.embed` rodados com um runtime instalado (um modelo
   pequeno feito com o `onnx` conferido contra o numpy; o SAM ViT-B exportado em ONNX em formas reais), o que estiver
   errado corrigido, e os pesos do SAM baixados uma vez para o cache (com o SHA-256 conferido) quando não dados.
   **Feito** (d17236f, f8e09fc): o ONNX Runtime 1.32 carrega pela API 17 sem mudança. O export do SAM ViT-B usado
   (vietanhdev/segment-anything-onnx-models, Apache-2.0) toma a imagem em HWC de 0 a 255 e normaliza por dentro: o
   encoder agora vê o formato da entrada (o caminho NCHW fica para os outros exports). O decoder desse export corta a
   máscara num quadro fixo de 683 × 1024: o tile vai redimensionado para caber nele. As sobreposições como o gerador
   automático do SAM (estabilidade, IoU previsto, área máxima, NMS das caixas, as máscaras maiores pintadas antes): numa
   cena sintética de 4 formas, cada uma vira um objeto com IoU de 0,994 a 1,000 (antes 0,19 a 0,97). O
   `objects.embed` confere com o numpy até 1e-5 (`tools/make_embed_reference.py`, um modelo de 684 bytes). Sem
   `encoder` e `decoder`, os pesos (75 MB quantizados, ou 348 MB com `weights` full) são baixados uma vez para o cache,
   o SHA-256 conferido, extraídos por um leitor de zip sobre o flate2 (sem dependência nova).
3. **O cluster pelo aplicativo**: os tiles do `segment.multiresolution` nos computadores que trabalham juntos do app
   (`computers.rs`), não só pelo `--coordinator` da CLI.
   **Feito**: `Computers::cluster()` dá o endereço e o código do pool do app só quando outros computadores trabalham
   para ele (o "Let other computers help" de sempre, nada novo nos ajustes); a execução monta o `Remote` (`runner.rs`,
   `engine_for`) e, sem conexão, fica aqui. Se os tiles falham no cluster, rodam aqui com um aviso, como as features. O
   teste com um pool do app em 127.0.0.1 e dois workers dá os mesmos rótulos e a mesma tabela.
4. **O `objects.features` no cluster**: as faixas de linhas como trabalhos dos workers, a tabela juntada no
   coordenador, a mesma tabela que aqui.
   **Feito**: `objects/rows.rs` (`FeatureJob`, `offload`): os grupos `spectral` e `shape` e a textura de cada camada
   como tarefas dos workers (os rótulos num arquivo temporário, os rasters pelo hash ou pelo disco compartilhado, como
   o trabalho `segment`), o `context` no coordenador; um segundo método do `TileCores`, o mesmo `--coordinator`. Sem
   cluster, com a imagem na memória, ou se o cluster falhar (com um aviso), tudo aqui. A tabela igual, coluna a coluna.
5. **Os rótulos em disco e a cena inteira**: um tipo `Labels` (na memória ou num arquivo mapeado quando passam de um
   limite), o pico de memória do `merge_tiled` e das features reduzido, e uma cena Sentinel-2 inteira (10 980 × 10 980,
   10 bandas) do disco segmentada e descrita abaixo de 4 GB de pico.
   **Feito**: `objects/labels.rs` (`Labels`, na memória ou num arquivo temporário mapeado pelo `memmap2` acima de
   1/16 da memória, `LABELS_DISK_BYTES`); `merge_tiled` grava o núcleo de cada tile direto num só mapa de rótulos (o
   deslocamento de cada tile somando os rótulos dos anteriores), sem os núcleos todos nem o mapa u64 das regiões, e
   roda tantos tiles juntos quanto cabem em 2 GB (`TILES_BYTES`); `Objects::from_fn`; o índice das células por objeto
   das features só quando um grupo o pede. A cena inteira (u16, tiles de 512, escala 300, 175 639 objetos, 8 threads):
   segmentação em 413 s e features em 233 s, pico de 2,07 GB (antes 5,29 GB). No Windows as páginas mapeadas tocadas
   contam no working set: o arquivo só deixa a memória ser devolvida, não baixa o pico medido.
6. **Os segmentadores que faltam** do eCognition, do OTB e do GRASS, os que fazem sentido aqui (por exemplo a
   *spectral difference*, a *multi-threshold* e a *contrast split* do eCognition, o crescimento de regiões do
   `i.segment` do GRASS e a otimização não supervisionada dos parâmetros do `i.segment.uspo`, os perfis morfológicos do
   OTB).
   **Feito, a primeira parte**: `segment.threshold` (o *multi-threshold*: uma banda cortada em limiares dados, ou nos
   de Otsu multinível, O(k·bins²) por programação dinâmica; cada peça conexa de uma classe é um objeto; a tabela com a
   classe e o output `thresholds`) e `segment.spectral_difference` (funde os vizinhos de uma sobressegmentação cuja
   diferença média ponderada das bandas fica abaixo de `max_difference`, sempre o par mais próximo primeiro, as médias
   refeitas a cada fusão; cada objeto na fila com o seu melhor par). Em 2000² × 4 bandas, 1 / 8 threads: o threshold
   com Otsu de 3 classes 0,10 / 0,06 s; o spectral difference sobre o SLIC 0,33 / 0,20 s, sobre um xadrez de 4 (250 mil
   objetos) 2,7 s. A *contrast split* fica de fora (a patente US7801361, até cerca de 2029).
   **Feito, a segunda parte**: `segment.region_growing` (o crescimento de regiões do `i.segment` do GRASS, descrito do
   manual, sem o código GPL: bandas escaladas a 0–1, o vizinho mais parecido e mútuo abaixo do `threshold`, passadas até
   nenhuma fundir, `seeds`, `bounds`, `minsize`) e `segment.quickshift` (Vedaldi & Soatto 2008: a densidade e as
   ligações por linhas em paralelo). Em 2000² × 4 bandas, 1 / 8 threads: o quickshift 19,6 / 2,7 s. O region growing
   fica num thread (a ordem embaralhada das visitas é o algoritmo): 27 s com `threshold` 0,05 e 72 s com 0,02 (1,6
   milhão de objetos), contra 11 s do multiresolution. Uma região grande, com dezenas de milhares de vizinhos, funde
   um por passada, por milhares de passadas. Ideias: por tiles como o multiresolution (os mesmos tiles e a costura), ou
   deixar uma região fundir mais de uma vez por passada (outra semântica, a medir na qualidade).
   **Feito, a terceira parte**: `segment.optimize` (o USPO de Espindola et al. 2006, como o `i.segment.uspo` do
   GRASS): qualquer `segment.*` que toma uma imagem roda em cada valor de um ajuste, em paralelo (em ondas que cabem
   em `TILES_BYTES`), e cada resultado é pontuado pela variância dentro dos objetos (pesada pelas células) e pelo I de
   Moran das médias entre objetos vizinhos, as duas escaladas entre as rodadas; vence a maior soma, ou o F de Johnson
   & Xie (2011) com `alpha` (a fórmula do `i.segment.uspo`: α acima de 1 pesa mais a variância). Só as rodadas não
   dominadas guardam os objetos. Multiresolution em 5 escalas, 1000²: 10,2 s com 1 thread, 3,0 s com 8. Ficam de
   fora os perfis morfológicos do OTB (não são um segmentador; um operador de raster, se for pedido).
7. ~~Tudo no terraflux-py~~: desistido (2026-11-19, o usuário: o OBIA fica só no aplicativo). O `python_script`
   continua avisando que esses nós não têm equivalente no terraflux-py.
8. **A segmentação o mais rápida possível, num núcleo e em paralelo** (pedido de 2026-11-19: um dos objetivos
   principais do OBIA é garantir que o processo seja o mais otimizado possível). Uma bateria fixa de imagens e o tempo
   de cada segmentador com 1 e com 8 threads, guardada aqui; o perfil do `segment.multiresolution` (a primeira passada
   sobre os pixels, o grafo de regiões, as alocações, a costura dos tiles, que hoje refaz a fusão sobre a imagem
   inteira) e dos outros; os mesmos objetos sempre que possível, e qualidade medida quando não. Os segmentadores novos
   da fase 6 já entram com o tempo medido.
   **Feito, o `segment.multiresolution` num núcleo** (cc01e11): cerca de 1,7 vez mais rápido, os mesmos objetos (os
   custos são os mesmos números, bit a bit). O custo de fusão era pouco do tempo: o resto era esperar a memória, pois
   as visitas são embaralhadas. Agora cada região guarda os termos do custo que só dependem dela (o n·sd de cada banda e
   os de forma), o custo de cada par fica nas listas de vizinhos até uma das regiões mudar, as listas ficam na região
   (`SmallVec`, sem heap por região) e as regiões das próximas visitas são buscadas antes (prefetch). A bateria é
   `crates/ops/tests/objects_bench.rs` (ignorada), em imagens sintéticas de 4 bandas, segundos com 1 / 8 threads:

   | segmentador | 1000² antes | 1000² depois | 2000² antes | 2000² depois |
   |---|---|---|---|---|
   | multiresolution, escala 20 | 2,34 / 3,92 | 1,37 / 1,39 | 19,5 / 19,6 | 11,1 / 11,3 |
   | multiresolution, escala 50 | 4,49 / 4,47 | 2,49 / 2,62 | 19,7 / 19,7 | 11,4 / 11,3 |
   | multiresolution, escala 50, tiles de 512 | 5,21 / 1,52 | 2,90 / 0,97 | 23,7 / 4,66 | 13,6 / 3,30 |
   | slic | 0,53 / 0,53 | | 2,13 / 2,01 | |
   | snic | 0,77 / 0,75 | | 5,20 / 5,15 | |
   | felzenszwalb | 0,86 / 0,83 | | 3,90 / 3,93 | |
   | watershed | 1,65 / 1,71 | | 9,29 / 9,40 | |
   | meanshift | 4,23 / 0,73 | | 18,2 / 3,17 | |

   **Feito, os outros segmentadores** (5508adf), os mesmos rótulos (a bateria imprime um hash deles, igual ao de
   antes): o SLIC atribui as células por linhas em paralelo; o Felzenszwalb monta e ordena as arestas em paralelo; o
   watershed e o SNIC só põem uma célula na fila quando ela vence todas as prioridades já postas para ela, numa fila
   4-ária compacta; a suavização, o gradiente, os componentes conexos de `Objects` e as arestas entre objetos nos
   threads; os tiles do multiresolution numa fila contínua (sem barreira por lote), os núcleos postos na ordem dos
   tiles. Segundos com 1 / 8 threads (medidos com outra compilação na máquina; medidos de novo abaixo):

   | segmentador | 1000² antes | 1000² depois | 2000² antes | 2000² depois |
   |---|---|---|---|---|
   | slic | 0,55 / 0,51 | 0,25 / 0,06 | 2,2 / 2,1 | 1,0 / 0,39 |
   | snic | 0,78 / 0,80 | 0,31 / 0,29 | 4,9 / 5,1 | 3,0 / 2,8 |
   | felzenszwalb | 0,79 / 0,79 | 0,34 / 0,20 | 3,7 / 3,8 | 3,0 / 1,5 |
   | watershed | 1,74 / 1,72 | 0,65 / 0,39 | 9,3 / 9,0 | 2,9 / 1,8 |
   | multiresolution, tiles de 512, 8 threads | 0,96 | 0,92 | 3,35 | 2,90 |

   **Medido de novo em 2026-11-23, com tudo o que foi feito acima** (a bateria completa, release, 8 threads de teto, nada
   mais meu rodando; imagens de 4 bandas, segundos com 1 / 8 threads, o melhor de 5 rodadas: a máquina não ficou quieta
   em todas, as rodadas 1, 2 e 5 levaram de 1,5 a 2 vezes o tempo das rodadas 3 e 4, e o melhor é o das quietas; a
   mediana das rodadas 3 a 5 fica a até 10% dele nas linhas de 1000²). Os hashes dos rótulos foram os mesmos nas 5
   rodadas e iguais aos de antes (por exemplo multiresolution 71a81c69746be0d9 / 9ee678ac1dfd31f5 / 761a9492149a440a em
   1000², watershed e086c942b441a075, region growing t0.05 920c612a67b41154):

   | segmentador | 1000² (1 / 8 threads) | 2000² (1 / 8 threads) |
   |---|---|---|
   | multiresolution scale20 | 1,27 / 1,23 | 6,02 / 5,89 |
   | multiresolution scale50 | 1,30 / 1,26 | 6,10 / 5,81 |
   | multiresolution scale50-tile512 | 1,91 / 1,07 | 9,36 / 5,17 |
   | chessboard default | 0,02 / 0,01 | 0,07 / 0,04 |
   | quadtree default | 0,20 / 0,14 | 0,81 / 0,55 |
   | slic default | 0,25 / 0,07 | 0,98 / 0,24 |
   | snic default | 0,31 / 0,29 | 1,90 / 1,82 |
   | felzenszwalb default | 0,38 / 0,22 | 1,59 / 0,94 |
   | watershed default | 0,26 / 0,14 | 1,18 / 0,71 |
   | meanshift default | 1,81 / 0,34 | 7,83 / 1,41 |
   | threshold otsu-3 | 0,02 / 0,01 | 0,10 / 0,05 |
   | threshold otsu-5-1024 | 0,04 / 0,02 | 0,14 / 0,07 |
   | threshold given-4 | 0,04 / 0,02 | 0,13 / 0,07 |
   | spectral_difference chess4-d8 | 0,26 / 0,25 | 1,42 / 1,37 |
   | spectral_difference slic-d8 | 0,04 / 0,03 | 0,14 / 0,10 |
   | region_growing t0,02 | 1,93 / 1,85 | 40,4 / 40,0 |
   | region_growing t0,05-min20 | 3,23 / 3,22 | 15,5 / 15,2 |
   | region_growing t0,05-n8-manh | 3,14 / 3,16 | 15,6 / 15,1 |
   | region_growing t0,05 | 3,30 / 3,33 | 15,5 / 15,4 |
   | region_growing t0,05-tile512 | 5,00 / 3,31 | 20,8 / 11,9 |
   | region_growing t0,05-min20-tile512 | 4,95 / 3,33 | 20,7 / 11,9 |
   | quickshift default | 2,11 / 0,50 | 8,34 / 1,52 |
   | quickshift k3-d8 | 1,00 / 0,22 | 4,04 / 0,76 |
   | optimize ms-5-scales | 6,80 / 2,80 | 29,8 / 29,0 |

   Contra as tabelas acima: o multiresolution sem tiles em 2000² foi de 11,1 / 11,3 s para 6,0 / 5,9 s (escala 20) e de
   11,4 s para 6,1 / 5,8 s (escala 50); o watershed de 2,9 / 1,8 s para 1,18 / 0,71 s; o SLIC de 1,0 / 0,39 s para 0,98 /
   0,24 s; o SNIC de 3,0 / 2,8 s para 1,90 / 1,82 s; o Felzenszwalb de 3,0 / 1,5 s para 1,59 / 0,94 s; o region growing
   com `threshold` 0,05 em 2000² de 27 s para 15,5 s. Uma coisa está pior: o multiresolution com tiles de 512 em 2000² com 8 threads está em 5,2 s, contra 3,3 s da
   tabela de antes (com 1 thread melhorou, de 13,6 para 9,4 s). Provavelmente é a costura que desfaz as peças que tocam as
   bordas (o que tirou a emenda visível; `seam_band` corta essa parte quase pela metade), medida depois daquela tabela;
   não medi a versão anterior para confirmar.

   Falta: o que segue sequencial domina com 8 threads (a fila do SNIC, o union-find do Felzenszwalb, os mínimos do
   watershed e, na costura do multiresolution, o `merge_all`, que é ~90% dela e não pode mudar a ordem; a soma das
   estatísticas no laço de `Regions::from_source` também, porque as somas em ponto flutuante têm de seguir a ordem das
   células para manter os bits).
   Feito: com `compactness` 0 o flood do watershed usa uma fila de baldes (um balde por valor distinto do gradiente, em
   ordem de chegada, achados por um bitmap de três níveis; o rótulo vai no enfileiramento e cada célula entra uma vez),
   com os mesmos rótulos do heap (mesmos hashes no bench e um teste com gradientes cheios de empates), e a ordenação do
   gradiente é feita uma vez para os mínimos e para o flood. Watershed 1000² / 2000² (1 / 8 threads, mediana de 4
   rodadas alternadas): 0,36 / 0,25 s e 1,67 / 1,19 s antes; 0,27 / 0,16 s e 1,32 / 0,96 s depois. O flood de 2000² em
   1 thread foi de uns 0,9 s para 0,35 s; o que resta de sequencial é o suavizado, a ordenação e o union-find dos mínimos.
   Feito (2026-11-22): em `Objects::indexed` (usado por todo `from_fn`) a lista de vizinhos de cada objeto e a tabela de
   ids são montadas em paralelo (nos 1,7 milhões de objetos que o `dissolve_at_borders` deixa, era ~105 ms dos ~145 ms
   do passo); `eval_all`, a ordenação das listas e o `refresh` de `from_source` em paralelo por região; o laço de
   `dissolve_at_borders` só visita as células ao lado das bordas; o `smooth` (suavizado dos segmentadores) não copia mais
   a imagem duas vezes e tem um caminho para a janela inteira sem células sem valor, com os mesmos bits (teste contra a
   versão simples); `objects.split` ordena o gradiente uma vez. Hashes iguais em todos os casos medidos. Com a máquina
   ocupada as rodadas variaram ±30% e o ganho na costura (uns 0,1 a 0,2 s de 6 s com 8 threads em 2000²) ficou dentro
   do ruído; o `smooth` deu -0,07 a -0,13 s em 2000² com 1 thread (slic 1,15 -> 1,02, snic 2,49 -> 2,32, watershed 1,37
   -> 1,30 s).
   Feito (2026-11-22, a costura): o que domina o multiresolution com tiles e 8 threads é o `merge_all` da costura
   (~2,6 de ~5,8 s em 2000²): 1,72 milhão das 4 milhões de células voltam a ser regiões soltas (as peças que tocam uma
   borda são desfeitas) e são refundidas na ordem embaralhada, tão caro por célula quanto o caso sem tiles; só 481
   regiões estão inertes, então não há o que pular com exatidão. No `merge_all` em si, buscas antecipadas (prefetch)
   das regiões vizinhas cujo custo falta e das listas dos vizinhos da fusão tiraram uns 10-20% numa thread em 1000²
   sem tiles, mesmos rótulos. Experimento `seam_band` (padrão 0 = o de hoje): desfazer só as células a `seam_band`
   células das bordas e manter o resto de cada peça. Com 1024², tiles de 256, 8 threads, imagem de Voronoi: escala 20,
   hoje 1307 objetos em 4,6 s, sem tiles 1285; `seam_band` 2 dá 1187 em 2,3 s e 8 dá 1180 em 3,0 s (-8% de objetos,
   fora do ruído de outra semente, ±1,5%), 28 (1x o lado típico) dá 1278 em 3,9 s; escala 40: 867 hoje, 861 em 1,6 s
   com 2 e 855 com 8, F contra o sem-tiles 0,868 -> 0,879/0,876; imagem suave: sem diferença nos objetos e -15 a -25%
   do tempo. O corte nas bordas fica abaixo do corte nas outras linhas em todos. Faixas finas ganham 2x mas
   fundem a mais em escalas pequenas; ainda não é o padrão.
   Feito (2026-11-22, region growing): no `merge_all` do `segment.region_growing` o tempo não está na cauda de passadas
   (2000², `threshold` 0,05: 1000 das 2689 passadas já levam 23,5 de 24,1 s; a cauda é 0,6 s) e sim nas primeiras: ~4M
   fusões a ~2,6 us (127M entradas de listas, ~82 ns cada, a maior parte esperando memória) e as visitas. Depois da
   passada 20 ficam ~40 mil regiões ativas e ~70 fusões por passada por ~2400 passadas (as vizinhas do gigante, que
   esperam por ele estampado): ~100M visitas sem efeito, que não dá para pular com exatidão (a ordem embaralhada usa o
   tamanho da lista). Exato: buscas antecipadas dos vizinhos que vão ter o vizinho mais próximo refeito na fusão (mesmos
   hashes, ~10-20% em 1000² com 1 thread, ruído alto). Testado e descartado: deixar uma região fundir mais de uma vez
   por passada (`chain`): na imagem do bench com `threshold` 0,05 deu 5277 objetos contra 1522 e 59 s contra 4,5 s (F
   0,78 contra 0,90 de outra semente); com 0,02, +3% de objetos e F 1,000, 5,2 -> 3,4 s. Pior onde devia ajudar:
   não entrou no código.
   Visto (2026-11-22, SNIC e Felzenszwalb), sem mudança exata que se meça. Felzenszwalb 2000² (8 vizinhos, 16M
   arestas, 1 thread): arestas 0,25 s, ordenação 0,67 s, união ~3,0 s, corte 0,42 s, `min_size` ~1,2 s. A união
   domina: as arestas vêm em ordem de peso e cada uma lê duas células longe no espaço (~190 ns por aresta mesmo com
   prefetch). Um radix sort não ganha (lê e escreve ~256 MB por passada, ~0,6 s, o mesmo do `par_sort_unstable`).
   Filtrar em paralelo as arestas candidatas da segunda passada, com ou sem prefetch de 32, ficou dentro do ruído.
   SNIC: a fila é um heap 4-ário de u128 (distância, ordem do push, célula) com centroides que mudam a cada célula.
   Uma fila de baldes teria de quantizar a distância e mudaria os rótulos. Uma chave de 64 bits perderia o desempate.
9. Depois, o bloco seguinte do PCA, do k-means e do SOM (abaixo).

### Qualidade de vida do OBIA (pedido de 2026-11-19)

O que o aplicativo já tem: a segmentação ao vivo do `segment.multiresolution` no Viewer (`live_segment.rs`), os
contornos e o preenchimento por classe (`objects_view.rs`), o inspetor, a seleção com vizinhos, pais e filhos e as
amostras por clique (`objects_inspect.rs`), o espaço de atributos com o histograma, a separabilidade e *Make a rule*
(`feature_space.rs`), a árvore de classes (`class_tree.rs`) e o tempo de cada nó depois de rodar. O que falta, na
ordem em que será feito, depois do bloco do PCA, do k-means e do SOM (um commit por item):

1. **A segmentação ao vivo para todos os segmentadores.** Hoje os sliders só valem para o `segment.multiresolution`.
   Com os segmentadores rápidos (em 1000², o SLIC em 0,06 s com 8 threads, o watershed em 0,4 s, o threshold em
   0,02 s), cada um ganha os seus dois ou três ajustes principais como sliders sobre a parte do mapa à vista: o
   `size` e a `compactness` do SLIC e do SNIC, o `k` e o `sigma` do Felzenszwalb, o `h` e a `compactness` do
   watershed, os limiares ou as `classes` do threshold, o `max_difference` do spectral difference, o `threshold` do
   region growing, o `kernel_size` e o `max_dist` do quickshift, os raios do mean shift. *Apply* grava no nó, um passo
   de desfazer, como hoje.
2. **O gráfico e o *Apply* do `segment.optimize`.** A tabela `scores` como gráfico em `charts.rs` (o objetivo, a
   variância e o I de Moran escalados contra o valor, o melhor marcado), como o cotovelo do PCA e a escolha do k; e
   um botão que grava o melhor valor no nó do segmentador (ou num `segment.*` novo ligado à mesma imagem).
3. **A edição manual dos objetos.** No inspetor: juntar os objetos selecionados, cortar um objeto por uma linha
   desenhada, apagar (o objeto vira sem objeto, ou é absorvido pelo vizinho mais parecido). As edições ficam no
   modelo, num nó `objects.edit` com a lista das operações (por coordenadas, não por números de objeto, que mudam
   quando a segmentação de cima muda), para que a análise continue reproduzível; um aviso quando uma edição não
   acha mais o seu objeto. É o que quem vem do eCognition faz o tempo todo para corrigir o segmentador.
4. **Comparar segmentadores lado a lado.** A mesma janela do Viewer segmentada em três ou quatro miniaturas
   (multiresolution, SLIC, watershed, Felzenszwalb, ou os escolhidos), cada uma com o número de objetos e o tempo;
   um clique numa troca o nó (ou cria um). Responde à primeira pergunta de quem começa: qual segmentador usar.
5. **A estimativa do tempo antes de rodar.** Das taxas da bateria (`crates/ops/tests/objects_bench.rs`, células por
   segundo de cada segmentador, com 1 e com N threads) e do tamanho da imagem, o nó diz antes de rodar "cerca de 5
   min com 8 threads; por tiles de 512, cerca de 1 min", e sugere os tiles quando a imagem é grande.
6. **O SAM na primeira vez.** Antes do download dos pesos, um aviso no aplicativo (75 MB, a licença Apache-2.0, a
   origem, onde ficam no cache), a barra de progresso e o cancelamento; e onde trocar para os pesos completos.
7. **O region growing por tiles**, como o multiresolution (os tiles com margem, a fila contínua e a costura), para
   tirar os 30 a 70 s das imagens grandes e usar os núcleos (hoje um thread só).
8. **Templates dos operadores novos**: a água pelo Otsu sobre o NDWI (`segment.threshold`), a escala escolhida
   sozinha (`segment.optimize`), e as classes não supervisionadas dos objetos (`ml.kmeans` ou `ml.som` sobre a tabela
   de atributos, junto com a etapa G do plano abaixo), rodando como estão e com `data` para o *Use my data*.

**Plano de implementação** (levantado no código em 2026-11-19). Todos editam o modelo como hoje o `live_segment::apply`
e o *Make a rule* do `feature_space`: os parâmetros do nó ou `editor::{add_node_named, connect, set_param,
insert_between}` (nós novos postos por `builder::beside`), depois `s.history.name(...)` e `s.edited()`, um passo de
desfazer (a cópia `before` volta se der erro); os testes desfazem com `builder::undo_redo`. A ordem, pelas
dependências: 6, 2, 1, 4, 3, 7, 5, 8.

- **6. O SAM na primeira vez (P).** O progresso e o cancelamento já funcionam (`deep.rs` chama `ctx.progress`, que
  chega pelo `runner.rs` à barra de estado; parar apaga o zip). Falta a confirmação antes: uma porta como as de
  disco e memória (`disk_ok`/`memory_ok` em `app.rs`), `Session::sam_ok`, posta em `start_run` (não em `run_all`, que
  as execuções até um nó e as automáticas pulam; numa automática, um aviso em vez da janela). A janela (o padrão do
  `confirm_window`, `egui::Modal`) diz o tamanho, a licença, a origem e a pasta do cache, com a escolha quant/full
  (grava `weights`). `weights::cached` (pública) diz se já estão no cache. `.choices(["quant", "full"])` no
  `weights`; o texto do progresso em MB, não MiB. Testes: `cached`; no `--test app`, com `TERRAFLUX_CACHE` vazio, a
  janela aparece e a execução não começa; *Cancel* não muda o modelo.
  **Feito** (Unreleased): `sam_view.rs` (a janela, `Session::sam_need`/`sam_ok` em `start_run`, o consentimento por
  variante em `sam_agreed` das configurações); só os `segment.sam` do nível de cima do modelo passam pela porta.
- **2. O gráfico e o *Apply* do `segment.optimize` (M).** `Kind::Scores` em `charts.rs` (`is_scores`: as colunas
  `value, objects, variance, moran, objective`): o objetivo e as duas medidas escaladas contra o valor, o melhor
  marcado, e o número de objetos num painel menor. `optimize::scaled` e `default_parameter` públicos. Um
  `optimize_view.rs` (`panel`) no ramo das tabelas do Viewer (`builder.rs`) e no `node_view.rs`: "melhor size = 12" e
  *Apply to...*, com os nós do mesmo segmentador sobre a mesma imagem e "um nó novo" (`add_node_named` + `connect`).
  Testes: `is_scores`; aplicar num nó existente e num novo, um passo de desfazer cada; no `--test app`, rodar,
  aplicar e conferir o parâmetro.
  **Feito**: `Kind::Scores` e `is_scores` em `charts.rs` (o objetivo, a variância e o I de Moran escalados por
  `optimize::scaled`, agora pública com `default_parameter`; o melhor marcado por um `VLine`, os objetos num painel
  menor; a tabela abre já como o gráfico); `optimize_view.rs` (`panel`, `apply`, `targets`) no ramo das tabelas do
  Viewer e no `node_view.rs`: "best scale = 4", *Apply to:* e *Apply*, grava o valor e os ajustes fixos do `params`
  num segmentador da mesma imagem ou num nó novo, um passo de desfazer. Testes: `charts.rs`, `optimize_view.rs` e
  `tests/optimize_view.rs`. Feito (2026-11-22): um clique no gráfico escolhe o valor candidato mais perto do ponteiro (`charts::nearest_value`, guardado por tabela e pelo hash dos scores em `score_pick`); o gráfico marca o escolhido com uma segunda linha no destaque, o painel diz "Chosen: v" (com "best" e *Back to the best*) e *Apply* grava o escolhido, num passo de desfazer chamado "<Parâmetro> v of <nó>".
- **1. A segmentação ao vivo para todos (G).** O worker do `live_segment.rs` (`Request`, `spawn`, só o último pedido,
  `stale`) serve a qualquer pedido: `Job::Multires` (o de hoje, com a curva do ESP) e `Job::Op { op, params, extra }`.
  Uma tabela `knobs(op)` dos sliders (parâmetro, faixa, log, inteiro, unidade em valores ou em células): `size` e
  `compactness` do SLIC e do SNIC, `k` e `sigma` do Felzenszwalb, `h` e `compactness` do watershed, `classes` do
  threshold, `max_difference` do spectral difference, `threshold` do region growing, `kernel_size` e `max_dist` do
  quickshift, os raios do mean shift. Em `ops/objects/window.rs`, `segment_window_op` roda qualquer segmentador pelo
  registro numa janela (como o `segment.optimize` já faz). Para a janela dar os objetos da imagem inteira: os que
  padronizam recebem a janela já em z-scores pela média e o desvio da imagem inteira, com `standardise` desligado; os
  ajustes em células divididos pelo passo de leitura; os lentos (region growing, quickshift, mean shift) com menos
  células; as entradas `objects`, `markers`, `seeds` e `bounds` cortadas na janela. O SAM e os `from_*` ficam de
  fora. Testes: `segment_window_op` na janela inteira igual à execução do operador (o hash dos rótulos), para cada um;
  `knobs` cobre todos; `apply` grava o `size` inteiro, um passo de desfazer; no `--test app`, o slider do SLIC mostra
  a prévia e o *Apply* grava.
  **Feito** (Unreleased): `live_segment.rs` (`knobs(op)`, `Job::Op`, `plan_for`: a imagem de até 1 milhão de células, 250 mil nos
  lentos, é segmentada inteira, senão a parte à vista), `segment_window_op` e `Whole` em `ops/objects/window.rs`, o parâmetro `ranges`
  do region growing. Os limiares do threshold vêm da imagem inteira (lida a cada poucas células); `h` e `compactness` do
  watershed e `k` do Felzenszwalb também são ajustados pelo passo.
- **4. Comparar segmentadores (M, depois do 1).** `segment_compare.rs`, uma janela aberta do painel da segmentação ao
  vivo: um worker roda os escolhidos um depois do outro pelo `segment_window_op` na mesma janela (com os ajustes de um
  nó existente daquela imagem, ou os padrões da tabela); miniaturas com o `objects_view::outline_image` sobre a
  imagem, o número de objetos e os segundos. *Usar* põe o segmentador no lugar do mostrado. Testes: os padrões por
  operador; no `--test app`, as quatro miniaturas e *Usar* criando o `segment.slic` ligado à imagem.
  **Feito** (Unreleased): `segment_compare.rs` (`section`, `use_op`, `params_for`; o botão *Compare...* no painel de `live_segment.rs`, a janela
  de todos os oito segmentadores, os quatro primeiros marcados, uma janela só para todos pelo `plan_for`, o worker um por vez, só o
  último pedido); *Use* religa quem lia o mostrado (reusa o nó do operador sobre a imagem, ou cria um com os ajustes das miniaturas), um passo de
  desfazer. Feito (2026-11-22): o menu de uma camada raster (mapa ou pilha em memória) tem "Compare segmenters...", que abre a mesma janela sobre aquela imagem (`segment_compare::open_for_layer`, desenhada por `layer_windows`); sem segmentador mostrado cada miniatura tem "Add" no lugar de "Use" (`add_op`: um nó lendo a imagem, com as configurações da miniatura, ao lado do nó da imagem, num passo de desfazer, e seus objetos mostrados no Viewer).
- **3. A edição manual dos objetos (G).** Um operador `objects.edit` (`ops/objects/edit.rs`): entradas `objects` e
  `image` (opcional, para a semelhança e a tabela); o parâmetro `edits` (`Type::Any`, uma lista JSON aplicada em
  ordem, como `objects.rules` guarda as classes): `{"merge": [[x, y], ...]}`, `{"cut": [[x, y], ...]}` (uma linha) e
  `{"delete": [x, y], "into": "none" | "neighbour"}`, por coordenadas no CRS da grade (a célula interior mais perto do
  centróide, como o modo de amostras escolhe: `objects_inspect::sample_cell`). Juntar renumera para o objeto do
  primeiro ponto; cortar rasteriza a linha, separa em componentes conexos e dá cada célula da linha a um pedaço
  vizinho; apagar deixa sem objeto ou junta ao vizinho mais parecido (as médias das bandas; sem `image`, a borda mais
  longa). Uma edição que não acha o seu objeto vira um aviso e uma linha da tabela `report`. No app, um bloco
  *Edit* no inspetor (`ObjectsUi::inspector`): *Merge* (dois ou mais selecionados), *Delete*, *Cut* (a linha pelo
  `draw::Sketch`); `objects_edit.rs::apply` acrescenta ao `objects.edit` que já lê aqueles objetos, ou cria um entre
  o segmentador e quem o lê (`editor::insert_between`). Testes: cada operação; as mesmas edições sobrevivendo a outra
  segmentação; o aviso; no app, inserir e desfazer; no `--test app`, juntar dois e contar um a menos.
  **Feito** (Unreleased): `ops/objects/edit.rs` (`objects.edit`, `neighbours` pelos objetos de entrada: 8 quando eles só são
  inteiros com 8), `app/objects_edit.rs` (`apply`, `steps`, `base_of`: de uma classificação sobe até o segmentador) e o bloco
  *Edit* de `ObjectsUi::inspector` (*Cut* pelo `draw::Sketch` de linha, posto e tirado pelo `builder.rs`). Juntar que não toca
  o primeiro objeto deixa os separados e avisa (`not touching`). Testes: `tests/objects_edit.rs`, `objects_edit.rs` e o
  `tests/objects_viewer.rs`.
- **7. O region growing por tiles (M/G).** Do `tiles.rs`, é do multiresolution: `tile_core` (o `merge_level` com o
  custo), os campos de `Run`, o `TileJob`/`TileCores` do cluster e a costura (`merge_level_src` sobre os pedaços). É
  genérico: o lado e a margem dos tiles, a semente, a geometria das janelas, a fila contínua (`spawn_tile`,
  `Ordered`), os deslocamentos dos rótulos, o limite `TILES_BYTES`, `Source::window`. Refatorar em `merge_tiled_with`
  (uma função por tile e uma de costura); o multiresolution mantém o caminho do cluster. O region growing: o núcleo
  `grow(...)` separado do `run`; as escalas das bandas (`scales`) da imagem inteira passadas a cada tile; os tiles com
  `minsize` 1 e o `minsize` só na costura; a costura pelo caminho de `seeds` (`Regions::from_labels`) sobre os
  pedaços; `bounds` e `seeds` cortados por tile; os parâmetros `tile` e `margin`. Testes: o mesmo hash com 1 e 8
  threads; em imagens de blocos constantes, por tiles igual a sem tiles; na imagem da bateria, o número de objetos a
  poucos por cento do sem tiles; uma linha nova na bateria.
  **Feito** (Unreleased): `merge_tiled_with` em `tiles.rs` (`Tile`, a fila contínua, os deslocamentos e o `TILES_BYTES`
  genéricos; `merge_tiled` do multiresolution é uma chamada dela, com o caminho do cluster, os hashes da bateria
  iguais), `grow` e `stitch` em `growing.rs`, os parâmetros `tile` (sem padrão: 0 é sem tiles) e `margin`. A costura
  une também os pedaços de um mesmo grupo de `seeds` (uma semente cortada por um tile volta inteira) antes de rodar
  com o `minsize`; só imagens na memória (do disco, não). Bateria 2000² x 4 bandas, `threshold` 0,05: sem tiles 16,2 s
  (1 thread) e 16,4 s (8); `tile` 512 em 14,1 s (1 thread) e 4,2 s (8), 2277 objetos contra 2590; com `minsize` 20,
  12,7 s e 4,4 s, 225 objetos contra 228. Os objetos não são os mesmos (a fusão gulosa depende da ordem): a diferença
  de 12% de objetos sem `minsize` está nos objetos minúsculos (o ruído de uma ou poucas células): com `minsize` 20 cai
  a 1%, e o padrão continua sem tiles (os testes sintéticos pequenos ficam abaixo de 6%).
- **5. A estimativa do tempo (M, depois do 7).** A bateria com `BENCH_RATES=1` grava
  `crates/ops/src/objects/rates.json` (lido por `include_str!`): por operador, segundos por milhão de células com 1 e
  com N threads e o expoente α tirado de 1000² e 2000² (o tempo não é linear: o multiresolution vai de 1,4 a 11,1 s
  para 4 vezes as células, α ≈ 1,5); por tiles, tiles × t(tile + 2 margens)^α / threads mais a costura. `estimate(op,
  params, células, bandas, threads)`. O tamanho da imagem antes de rodar vem do valor da entrada, se já rodou, ou do
  `Walk` do `memory.rs`, que já resolve a grade de cada nó pelos cabeçalhos dos arquivos (o `memory::Estimate` passa a
  levar as grades). No `node_view.rs`, ao lado do tempo da última execução: "cerca de 5 min com 8 threads; por tiles
  de 512, cerca de 1 min", e *Use tiles*. As taxas são de uma máquina: depois, corrigidas pela razão entre o tempo
  visto e o previsto. Testes: todo segmentador de imagem tem uma taxa; a estimativa cresce com células^α; no app, um
  nó de 4000² mostra a estimativa e *Use tiles* grava `tile`.
  **Feito** (Unreleased): `ops/objects/rates.rs` (`estimate`, `exponent`; `rates.json` lido por `include_str!`, escrito por `BENCH_RATES=1` na bateria, que ganhou o chessboard e o quadtree), `app/time_estimate.rs` (`line`, `use_tiles`, `learn`), `memory::Estimate::grids` e `Settings::time_ratios`. O tempo é cells^alpha (alpha de 1000² e 2000²: multiresolution 1,17, region growing 1,07), os threads por Amdahl (1 e 8) e os tiles ajustados da corrida por tiles da bateria (trabalho dos tiles que os threads dividem, mais a costura que não dividem). Não escala por parâmetros: o `scale` do multiresolution muda 3% de 20 para 50, e o `threshold` do region growing vai para lados opostos em 1000² (3,7 s a 0,05, 2,2 s a 0,02) e em 2000² (17 s e 41 s). Ignora também o conteúdo da imagem, `from`, `seeds` e `bounds`, e as bandas contam num fator linear aproximado. *Use tiles* aparece a partir de 30 s. A correção pela máquina guarda a última razão visto/previsto de cada operador nas configurações. As taxas vêm de uma medição com ruído (outro agente compilando a -j 4 ao mesmo tempo): medir de novo antes do release. Testes: `rates.rs`, `time_estimate.rs` e `tests/time_estimate.rs` (4000², sem rodar).
- **8. Templates (P; M com a mudança no `objects.classify`).** No grupo dos objetos de `templates.rs`, com `data` na
  imagem: a água pelo Otsu sobre o NDWI (`raster.calc` → `segment.threshold` com 2 classes → a classe da água →
  `objects.to_vector`); a escala escolhida sozinha (`segment.optimize` → o gráfico do item 2); as classes não
  supervisionadas dos objetos (as features → `ml.kmeans` na tabela), que pede ao `objects.classify` aceitar os modelos
  `KMeans` e `Som` (a etapa G do plano abaixo). Testes: `the_object_templates_run` estendido (a água ≥ 0,9 com a
  verdade; o optimize dá `value`; k classes) e os testes de todos os templates.
  **Feito** (Unreleased): `templates.rs`, no grupo dos objetos, "Water by Otsu on the NDWI" (`raster.calc` do NDWI, `segment.threshold` com
  2 classes, `objects.rules` com a classe 2 como `water` e a 1 como `land`, `objects.to_vector`) e "The scale chosen by itself"
  (`segment.optimize` de 2 a 20, de 2 em 2, com o `scores` que abre como o gráfico e o `value`; a descrição diz que o *Apply* grava
  a melhor escala num segmentador), ambos com `data` na imagem; o terceiro, "Unsupervised classes of objects", já existia.
  `the_object_templates_run` estendido (a água concorda 0,975 com a verdade desenhada; o optimize dá `value` e 10 escalas); no guia.

**Decisões tomadas** (padrões; o usuário pode mudar):
1. O region growing ganha o parâmetro opcional `ranges` (as faixas das bandas para a escala de 0 a 1), que a janela
   ao vivo e os tiles passam com as da imagem inteira, e que serve também para comparar datas.
2. O *Apply* do `segment.optimize` grava o valor escolhido e os ajustes fixos do `params` do optimize (o que foi
   pontuado); a dica do botão diz o que muda.
3. O `objects.edit` vai logo depois do segmentador (antes das features e da classificação, que rodam de novo) e dá
   objetos sem classes nem níveis; as edições ficam nos parâmetros (um vetor de pontos e linhas só se alguém precisar
   de milhares).
4. *Usar* na comparação põe o novo segmentador no lugar do mostrado (quem o lia passa a ler o novo; o antigo fica
   desligado no modelo), um passo de desfazer.
5. O consentimento do download do SAM é lembrado por máquina, para cada variante (quant e full).
6. Os tiles do region growing são opcionais (`tile`, sem padrão), para não mudar os resultados de hoje; a estimativa
   do item 5 os sugere quando a imagem é grande.

### O que falta do eCognition (pedido de 2026-11-19)

Comparado com o eCognition Developer (os algoritmos do *Reference Book* e as suas features), o Terraflux já tem os
segmentadores (todos menos a *contrast split*, fora pela patente US7801361), os níveis e a hierarquia, as features
espectrais, de forma, de textura (GLCM) e de contexto (a borda relativa e a distância a uma classe, os vizinhos, o pai
e os filhos), as regras fuzzy e a árvore de classes, o vizinho mais próximo e os classificadores, o *grow*, o *merge*,
o *remove small* e o *reclassify*, a acurácia, o temporal, o espaço de atributos (com a separabilidade), as amostras
por clique, o ESP e o USPO. Faltam seis coisas que aparecem em projetos reais, da mais para a menos importante:

1. **As camadas temáticas na segmentação.** No eCognition, a segmentação respeita os limites de um vetor ou de um mapa
   (lotes, quadras, talhões, estradas): nenhum objeto atravessa um limite, e os atributos da camada (a classe do lote)
   servem às regras. Hoje só o `segment.region_growing` tem `bounds` (um mapa); o multiresolution, o mais usado, não.
2. **Dividir objetos que se tocam.** A *watershed transformation* do eCognition separa objetos grudados pela distância
   à borda (copas de árvores, prédios, células, cupinzeiros). O nosso `segment.watershed` segmenta a imagem; não divide
   objetos existentes.
3. **Crescer, encolher e alisar objetos célula a célula.** O *pixel-based object resizing* (crescer ou encolher com uma
   condição sobre as células), a abertura e o fechamento dos objetos, tapar buracos e a *border optimization* (cada
   célula da borda vai ao objeto mais parecido). O `objects.grow` cresce por objetos vizinhos, não por células.
4. **A forma pelo esqueleto.** O comprimento da linha principal, a largura, a curvatura e os ramos: o que separa
   estradas, rios, cercas vivas e canais de manchas de mesma área. Não há nenhuma.
5. **Os extremos locais.** O *find local extrema*: o objeto de maior (ou menor) valor de um atributo num raio, como os
   topos das copas ou o ponto mais fundo. O *find enclosed by class* já sai da borda relativa (`rel_border_<c>` = 1),
   só falta dizê-lo.
6. **A fusão pelo objeto resultante.** A *image object fusion*: juntar um objeto ao vizinho só quando o objeto juntado
   melhora uma medida (a compacidade, a média de uma banda). O `objects.merge` por condição olha os dois objetos, não o
   resultado.

Ficam de fora: treinar CNNs dentro do programa (o eCognition usa o TensorFlow; pede um runtime de treino, e o SAM, os
embeddings e o `ml.fit` cobrem boa parte); o *template matching* (nicho); as nuvens de pontos LiDAR (o Terraflux não
tem o tipo; seria um bloco à parte); a *contrast split* e "imprimir" uma rede de objetos em outra (as patentes
US7801361 e US8319793, até 2029 e 2031).

**Plano de implementação** (levantado no código em 2026-11-19). Valem para todos:
- Cada operador novo em `crates/ops/src/objects/` (registrado no `objects/mod.rs`), com a assinatura descrita; as
  entradas em `tips.rs` e `phrases.rs`; na lista "sem equivalente" do `python/models.rs` (o OBIA não vai ao
  terraflux-py); no `essentials.rs` se tiver mais de seis parâmetros; as referências no `methods.rs`; o tópico
  `objects` do guia do MCP e o tutorial *from-ecognition* (a tabela algoritmo do eCognition → operador) atualizados;
  o `website/data/ops.json` no release.
- Os que percorrem objeto por objeto (2, 3, 4, 6) rodam os objetos em paralelo com o rayon sob o teto central de
  núcleos (nunca todos), os maiores primeiro para equilibrar, e montam o resultado na ordem dos ids: o mesmo resultado
  com 1 e com 8 threads, testado. Um auxiliar comum, `objects/each.rs`: a caixa e a máscara de cada objeto (com uma
  borda de uma célula), aproveitando o que o `features.rs` já usa para percorrer os objetos.
- Os que mudam a geometria (2, 3, 6) mantêm as classes (um pedaço herda a do objeto) e perdem os vínculos de nível,
  com um `ctx.warn` quando a entrada os tinha (como o `objects.edit`, decisão 3 da qualidade de vida); a tabela sai de
  novo com o `stats_table` quando há `image`.
- Cada um ganha linhas na bateria `objects_bench.rs` (1000² e 2000², 1 e 8 threads) e os números no IDEAS.
- Um commit por item, nesta ordem: 1, 2, 3, 4, 5, 6, e por fim o 7 (templates e documentação).

- **1. As camadas temáticas (M). Feito (2026-11-19).** *1a:* `zones.rs`, o `bounds` (mapa, vetor ou objetos) no
  multiresolution (com `from`, por tiles, pelo cluster e ao vivo) e no region growing. *1b:* o `bounds` nativo no
  Felzenszwalb (as arestas entre zonas fora do grafo) e no spectral difference (corta os objetos dados pelas zonas e só
  junta dentro da zona); `split_by_zones(objects, zones, neighbours)` (o `neighbours` a mais, para o threshold com 8)
  no SLIC, SNIC, watershed, quickshift, mean shift, threshold, chessboard e quadtree, como um passo depois, com a
  tabela refeita dos objetos finais (devolve os mesmos objetos quando nenhum cruza uma zona); a segmentação ao vivo já
  passava o `bounds` de todos (`extras_of`) e a comparação passa o `bounds` do nó mostrado a cada segmentador; o
  `thematic` e o `field` do `objects.features` (`th_mode`, `th_share`; um atributo de texto vira códigos com os nomes
  nos rótulos da tabela). Testes: `crates/ops/tests/objects_thematic.rs` e os de `zones.rs`.
  - Um auxiliar `objects/zones.rs::zones(args, "bounds", grid) -> Vec<u64>` (vazio = sem limites) lê o `bounds`
    como mapa (uma zona por valor, como hoje no region growing, `growing.rs` ~863), como vetor (cada feição uma zona,
    queimada pelo `vector::burn` com o índice da feição; as células fora de todas formam uma zona) ou como objetos (cada
    objeto uma zona). O region growing passa a usá-lo (e aceita vetores e objetos).
  - `segment.multiresolution` ganha a entrada opcional `bounds`. Com `from`, os objetos de partida são cortados
    pelas zonas (os componentes conexos de (objeto, zona)); sem `from`, cada célula já é uma região. Em
    `Regions::from_source` (`multiresolution.rs` ~170) as listas de vizinhos pulam os pares de células de zonas
    diferentes: o perímetro continua contando essas arestas (é contado à parte, ~205), então o custo de forma fica
    certo, como se a região encarasse outro objeto. Bit a bit igual ao de hoje sem `bounds` ou com uma só zona.
  - Por tiles (`tiles.rs`): as zonas cortadas por janela, como o `Source::window`; a costura (`merge_level_src` com os
    pedaços como `from`) recebe as zonas de novo; o caminho do cluster (`TileJob`) leva as zonas da janela.
  - Os outros segmentadores: nativo no Felzenszwalb (as arestas entre zonas fora do grafo) e no spectral difference
    (só junta dentro da zona); nos demais (SLIC, SNIC, watershed, quickshift, mean shift, threshold, chessboard,
    quadtree) um passo depois, `split_by_zones(objects, zones)`, que renumera pelos componentes conexos de (rótulo,
    zona) em tiras paralelas (`Objects::from_fn`). A segmentação ao vivo e a comparação cortam o `bounds` na janela
    (`window.rs` já corta o do region growing).
  - Os atributos da camada: `objects.features` ganha a entrada opcional `thematic` (mapa ou vetor, com `field` para o
    atributo do vetor): por objeto `th_mode` (o valor mais comum) e `th_share` (a sua parte das células). Com `bounds`
    da mesma camada, `th_share` é 1 e `th_mode` é a zona.
  - Testes: nenhum objeto atravessa uma zona (mapa, vetor e objetos); uma zona só igual a sem `bounds`, bit a bit;
    por tiles, o mesmo com 1 e 8 threads e o caminho do cluster (`terraflux-dist --test tiles`); o passo depois nos
    outros; `th_mode`/`th_share`; o region growing com um vetor.
- **2. Dividir objetos: `objects.split` (M). Feito (2026-11-19).** `split.rs` e `each.rs` (a caixa com borda e a máscara de cada objeto, `each` em paralelo, os maiores primeiro, o resultado na ordem dos ids), `minima` e `flood` do watershed passaram a `pub(super)` (o hash do `segment.watershed` não mudou). O `h` padrão é uma célula pela distância e 0,05 da amplitude do mapa pelo `surface`; o `neighbours` (4 ou 8, padrão 8) liga as células de um pedaço; sem nada a dividir devolve os mesmos objetos (com os níveis e a tabela). As células sem valor no `surface` (NaN) entram no pedaço mais próximo. O parágrafo da patente é a seção 8 de `docs/patents-objects-resolve.md` (os DOIs conferidos no Crossref). Bateria `objects_split_bench` (blobs que se tocam; 1 e 8 threads, o mesmo hash): 1000² em 0,05 s e 0,01 s, 2000² em 0,20 s e 0,05 s (distância; o `surface` e o `max_neck` na mesma ordem). Testes: `crates/ops/tests/objects_split.rs`.
  - Entradas `objects`, `surface` (mapa, opcional: um nDSM, o brilho) e `image` (opcional, para a tabela). Parâmetros:
    `by` (`distance`, o padrão: a distância à borda do objeto; ou `surface`, os picos do mapa são os centros, `invert`
    para centros escuros), `classes` (padrão: todos), `h` (a altura que um pico precisa acima da sela para ser um
    pedaço: unidades do mapa para a distância, do `surface` no outro; padrão uma célula), `min_size` (células; um
    pedaço menor volta ao vizinho de borda mais longa), `max_neck` (o *length factor* do eCognition: um corte só fica
    se for mais curto que `max_neck` × a largura do pedaço menor, duas vezes a sua maior distância à borda), `sigma`
    (suavizar o `surface`, padrão 0).
  - Por objeto, na sua caixa: a distância pelo `linkage::edt` (exato, Felzenszwalb e Huttenlocher), com as células
    fora do objeto como sítios, em unidades do mapa; os marcadores pelo `watershed::minima` sobre a distância negada
    (as h-minima por persistência que já existem) e a inundação pelo `watershed::flood`, os dois sobre um `Prep` cujo
    `valid` é a máscara do objeto (passam a `pub(super)`). Saídas `objects`, `table` e `report` (por objeto dividido,
    os pedaços).
  - Patente: fica perto da US7801361 (filtrar as células dos objetos de uma classe para uma camada nova e formar dela
    uma segunda rede). O desenho evita os dois elementos: a distância é uma conta dentro do operador, não uma camada,
    e os pedaços substituem os objetos no mesmo nível. A técnica anterior: o *declumping* do CellProfiler (Carpenter et
    al. 2006, Genome Biology 7:R100), Vincent e Soille (1991), o *Watershed* binário do ImageJ. Um parágrafo no
    `docs/patents-objects-resolve.md` antes do release.
  - Testes: dois discos que se tocam viram dois, cortados no gargalo; um disco não se divide; `max_neck` segura um
    haltere de gargalo largo; `min_size`; `classes`; `surface` com dois picos; o mesmo com 1 e 8 threads; células NaN;
    um objeto que ocupa a imagem toda.
- **3. Crescer, encolher e alisar: `objects.resize` (M/G). Feito (2026-11-19).** `resize.rs` (`each.rs` ganhou `each_with`, para caixas de rótulos que mudaram). O `grow` exato é o `edt` global com os objetos escolhidos como sítios (as células livres a até `distance` tomam o objeto mais perto; o empate é o do `edt`, determinístico, não o do id); o `shrink` é o `edt` por objeto na sua caixa (cada célula do objeto a até `distance` de uma célula que não é dele; a borda da imagem não conta, menos no `close`, onde conta como fora, senão o que cresceu até ela ficaria). `distance` exato em unidades do mapa, `steps` vezes a maior célula; no caminho geodésico a `distance` vira anéis inteiros. `into` vale no `grow`, no `close` e no `fill` (uma classe tomada que também é escolhida não é tomada); o `mask` (NaN e 0 não deixam) vale no `grow`, no `close` e no `fill` (as células de um buraco têm de ser permitidas). O `open` regrow só sobre as células originais do mesmo objeto; o `close` só devolve as células acrescentadas (a primeira fileira da boca de uma fenda fica de fora, como no fechamento matemático da cruz). O `fill` usa a vizinhança oposta à dos objetos (8 para objetos de 4). O `smooth` dá o voto ao "sem objeto" também, mas as células sem objeto nunca mudam (nada se acrescenta por fora); o empate fica com o rótulo da própria célula. O `refine` só move células dos objetos escolhidos, para qualquer vizinho (os escolhidos ou não), passa a passa estilo Jacobi, e para quando nada se move; uma célula com NaN em alguma banda fica. Um pedaço que sobra de um objeto vira objeto com a mesma classe (uma ilha de uma célula também). `report`: `object`, `before`, `after`, `pieces`. Referências no `methods.rs`: Serra 1982 (livro, sem DOI), Soille 2004 (2ª ed., DOI 10.1007/978-3-662-05088-0; o Crossref dá 2004, não 2003) e Felzenszwalb e Huttenlocher 2012 (10.4086/toc.2012.v008a019). Bateria `objects_resize_bench` (1 e 8 threads, o mesmo hash): grow (3) 1000² em 0,03 s e 0,03 s, 2000² em 0,14 s e 0,12 s; open (2) 1000² em 0,04 s e 0,03 s, 2000² em 0,17 s e 0,13 s; refine (3 passadas) 1000² em 0,06 s e 0,03 s, 2000² em 0,23 s e 0,11 s. Testes: `crates/ops/tests/objects_resize.rs`.
  - Entradas `objects`, `mask` (mapa, opcional: a condição, as células com valor diferente de 0, feita pelo
    `raster.calc`, por exemplo `ndvi > 0.3`) e `image` (para o `refine` e a tabela). Parâmetros: `mode` (`grow`,
    `shrink`, `open`, `close`, `fill`, `smooth`, `refine`), `classes` (os objetos que mudam), `distance` (unidades do
    mapa) ou `steps` (células), `into` (no `grow`: `none`, só as células sem objeto, o padrão, ou nomes de classes cujas
    células podem ser tomadas, os *candidatos* do eCognition), `max_hole` (no `fill`, em células; padrão todos os
    buracos fechados).
  - Sem `mask` nem `into`, o `grow` e o `shrink` são euclidianos exatos numa passada: o `linkage::edt` com as células
    dos objetos escolhidos como sítios dá a cada célula livre a distância e o objeto mais perto (crescer até `distance`
    é tomar as de distância menor); encolher é o `edt` de fora para dentro. Com `mask` ou `into` o crescimento tem de
    ser geodésico: anel por anel (`steps`), em paralelo por linhas com dois buffers, cada célula ao objeto com mais
    vizinhas, o empate para o id menor. `open` = encolher e crescer de volta só sobre as células originais; `close` =
    crescer e encolher só as células acrescentadas; `fill` = os componentes conexos de células sem objeto (ou de
    `into`) cercados por um só objeto, sem tocar a borda da imagem, até `max_hole`; `smooth` = a moda dos rótulos num
    disco de raio `distance` entre o objeto e os seus vizinhos (nenhum objeto novo); `refine` (a *border
    optimization*) = cada célula da borda vai ao objeto vizinho de médias mais perto (em z-scores), `steps` passadas
    ao estilo Jacobi (as médias atualizadas entre passadas, então paralelo e determinístico).
  - Um objeto partido em dois pelo `shrink` ou pelo `open` vira dois objetos, com a mesma classe.
  - Testes: um quadrado cresce 2 células; o disco euclidiano exato; dois objetos dividindo um vão pelo mais perto, o
    empate determinístico; a `mask` segura; o `open` corta uma ponte de uma célula; o `close` fecha um vão; o `fill`
    pelo tamanho; o `refine` leva uma borda errada à borda verdadeira numa imagem de duas regiões; 1 e 8 threads;
    NaN.
- **4. A forma pelo esqueleto (M). Feito (2026-11-20).** `skeleton.rs`, o grupo `skeleton` do `objects.features` (fora do `basic`; o `all` o inclui), e o parâmetro novo `spur` (padrão 3, 0 não poda); `features_give` e o `feature_of` conhecem o prefixo `skel_`. Por objeto, na caixa com borda (`each`), com os buffers de cada thread reaproveitados (`thread_local`). O Zhang e Suen roda sobre uma lista de células de borda (o custo segue o contorno, não a área; o resultado não depende da ordem); depois saem as células que não mudam a topologia (os degraus e as diagonais de duas células que o Zhang e Suen deixa; as pontas ficam), e um quadrado 2×2, que o afinamento apaga inteiro, guarda a célula mais perto do meio. A poda corta, a partir de cada ponta, as células até uma junção quando são menos de `spur`. Ponta = uma só corrida de vizinhas em volta da célula (número de cruzamento 1), junção = 3 ou mais; células de junção que se tocam são uma junção só; os ramos são os componentes sem as células de junção, ligados só pelos passos do grafo (um passo diagonal some quando uma das duas células vizinhas dos dois está no esqueleto; sem isso os braços de uma cruz se juntariam pelo canto). O anel conta 1 ramo, 0 pontas, 0 junções, e a linha principal dele é o caminho mais curto mais longo (meia volta). A linha principal é o diâmetro do grafo (duas buscas de Dijkstra, passos de largura, altura e diagonal da célula em unidades do mapa, então célula não quadrada vale). A curvatura (refeita em 2026-11-20: a média móvel de janela 5 deixava o degrau do raster de uma reta inclinada contar como giro, 0,085 rad/m a 25°, e 0,048 num quarto de círculo de raio 64 em vez de 0,0156): a linha principal é simplificada por Douglas e Peucker (1973, Cartographica 10(2):112-122, DOI conferido no Crossref: 10.3138/FM57-6770-U75U-7727) a uma célula de tolerância, e a curvatura é a soma dos ângulos nos vértices da polilinha sobre o comprimento dela; uma curva suave (giro < 0,5 rad) no primeiro ou no último vértice ganha a metade do giro em cada ponta (a polilinha perde a metade dos segmentos das pontas), uma quina não. Medido: retas a 0°, 25°, 45°, 63° dão 0; arcos de raio 20, 40, 64 dão 1,013, 0,990 e 0,975 vezes 1/r; o anel de raio 16, 0,89. `skel_curvature_sd` é o desvio, pelo comprimento, do giro por unidade de comprimento (pedaços: meio segmento de cada ponta e o trecho de cada vértice). Objeto de uma só célula ou comprimento 0: `skel_width` e `skel_length_width` NaN. Zhang e Suen (DOI conferido no Crossref: 10.1145/357994.358023) ficam na descrição do operador e não no `methods.rs`: ele casa os métodos pelo operador, e todo `methods` de um modelo com `objects.features` citaria o artigo mesmo sem o esqueleto. Bateria `objects_skeleton_bench` (blobs que se tocam; 1 e 8 threads, o mesmo hash): 1000² em 0,01 s e menos de 0,01 s, 2000² em 0,05 s e 0,01 s. Testes: `crates/ops/tests/objects_skeleton.rs`.
  - Um grupo novo `skeleton` em `features.rs` (`Group { name: "skeleton", ... }`), fora do padrão (pedido em
    `features`), com as colunas `skel_main_length` (o caminho mais longo, em unidades do mapa, a diagonal √2),
    `skel_width` (área / comprimento principal), `skel_length_width`, `skel_branches` (os segmentos entre pontas e
    junções), `skel_ends`, `skel_junctions`, `skel_branch_length` (média), `skel_curvature` (a soma dos ângulos de
    giro ao longo da linha principal, suavizada numa janela de poucas células, por unidade de comprimento) e
    `skel_curvature_sd`. A busca dos atributos pelo nome (`features.rs` ~250-280) conhece o prefixo.
  - Por objeto, na caixa com borda: o afinamento de Zhang e Suen (1984, CACM 27(3):236-239), com a limpeza dos
    degraus diagonais; as pontas mais curtas que `spur` células (padrão 3) podadas; o grafo das células do esqueleto
    (grau 1 ponta, 3 ou mais junção); a linha principal pelo diâmetro (duas buscas de Dijkstra com pesos 1 e √2); um
    ciclo não quebra a conta (a linha principal *no cycles* do eCognition).
  - O eCognition faz o esqueleto do polígono (por uma triangulação de Delaunay), o nosso é do raster: os valores
    diferem um pouco; a documentação diz isso.
  - Testes: uma linha de 1×50 (comprimento 49 células, um ramo, curvatura 0); um retângulo de 10×100 (comprimento
    perto de 90, largura perto de 10); um L (90° de curvatura); uma cruz (4 ramos, 1 junção); um anel; o tamanho da
    célula; 1 e 8 threads.
- **5. Os extremos locais: `objects.extrema` (P). Feito (2026-11-20).** `extrema.rs`. O `attribute` é sempre avaliado como expressão do `objects.calc` (`calc::evaluate`; um nome de coluna é uma expressão), e um texto é recusado. Decisões onde o plano calava: o `radius` e o `steps` juntos são recusados (como no `resize`); sem nenhum vale `steps` 1; o raio é inclusivo (`<=`, com folga de 1e-9) e mede entre os centróides (média das células, em unidades do mapa); os passos andam por todos os objetos (a busca em largura atravessa também os que não são candidatos), mas só os candidatos (`among`) e com valor são comparados; um objeto sem valor não é extremo nem barra os vizinhos; um objeto sozinho (sem candidato por perto) é extremo; `strict` desligado: um empatado com o maior também é extremo (todos os do platô). A classe dada é criada quando não existe (com a coluna de pertinência em 1 nos extremos e 0 nos outros, se os objetos tinham pertinências), os outros objetos mantêm classe, níveis e tabela; a coluna `extremum` vale 1, 0, ou NaN para o objeto que não foi comparado (fora do `among` ou sem valor). Sem referência no `methods.rs` (não é um método de artigo; a vizinhança é a do eCognition). Aviso quando nenhum candidato tem valor. Bateria `objects_extrema_bench` (objetos dos blobs crescidos até se tocarem, atributo `cells`; 1 e 4 threads, o mesmo hash; a máquina tem o teto de 4 núcleos, então o 8 não foi rodado): 1000² (737 objetos) e 2000² (3047) em menos de 0,01 s para `steps` 1, `steps` 3 e `radius` 40, com 1 e com 4 threads. Testes: `crates/ops/tests/objects_extrema.rs`.
  - Parâmetros: `attribute` (uma coluna da tabela ou uma expressão do `objects.calc`), `kind` (`max` ou `min`),
    `radius` (unidades do mapa, entre centróides) ou `steps` (vizinhos topológicos, padrão 1), `among` (as classes
    candidatas, padrão todas), `class` (o nome dado aos extremos, padrão "maximum"/"minimum"), `strict` (o *exclude
    equal* do eCognition: um empate não é extremo, o padrão). Saídas: os objetos com a classe nos extremos (os outros
    mantêm a sua) e a tabela com a coluna `extremum`.
  - O raio por uma grade de baldes do tamanho do raio; os passos por busca em largura sobre `neighbours_of`;
    paralelo por objeto.
  - O *find enclosed by class*: sem operador; uma dica, um exemplo no guia e no tutorial (`rel_border_<c> == 1`, e
    para várias classes a soma das bordas).
  - Testes: uma grade 3×3 de objetos com o máximo no centro; raio contra passos; `among`; empates; atributo NaN.
- **6. A fusão pelo resultado: `objects.fuse` (M). Feito (2026-11-20).** `fuse.rs` (o `regroup` do `levels.rs` passou a `pub(super)`). Cada grupo guarda as células, o perímetro em arestas de célula, a caixa e, por banda, a contagem, a soma e a soma dos quadrados (menos o centro da banda, como o `stats_table`, na mesma ordem das células); os atributos do par saem em tempo constante. Os nomes que o `fitting` entende: `cells`, `area`, `perimeter`, `length`, `width`, `length_width`, `compactness`, `shape_index`, `border_index` e, com `image`, `mean_<banda>` e `sd_<banda>`; qualquer outro nome (textura, esqueleto, contexto, `nb.*`, `border_to`, `a.x`) é recusado com a lista. Decisões onde o plano calava: (1) o comprimento e a largura são os lados do retângulo mínimo girado, como no `features` (ver "Retângulo girado" no fim; até 2026-11-20 era a caixa alinhada aos eixos); o perímetro usa o lado médio da célula (exato para células quadradas). (2) O `improve` aceita o empate (o juntado tão bom quanto a semente, com tolerância de 1e-12), senão dois retângulos nunca fariam um retângulo; um valor NaN da semente não deixa juntar. (3) O `threshold` é "no mínimo" para `goal` max e "no máximo" para min; vale junto com o `improve` ou sozinho (`improve` falso). (4) Um objeto pode ser semente e candidato (todas as classes): cada direção é uma proposta, o valor do par é o mesmo, a classe é a da semente da proposta aceita (empate de valor: a semente maior, depois o id menor); com classes diferentes só a semente propõe. (5) `best`: cada semente propõe o melhor candidato (empate: o id menor); `all`: toda proposta que qualifica; nos dois, as propostas são tomadas do melhor valor para o pior e um par só quando nenhum dos dois juntou na passada (os pares disjuntos). Não há "melhor mútuo" estrito: o par mútuo é o primeiro da fila quando é o melhor valor, e uma semente cujo melhor foi tomado espera a próxima passada (`best`) ou cai no seguinte na mesma (`all`). (6) A classe e as pertinências do juntado são as do objeto da semente que ficou à frente do grupo; `unclassified` vale como nome de classe em `seeds` e `candidates`. (7) `max_passes` padrão 100; as passadas só avaliam de novo o que a anterior mudou (ver abaixo). (8) Saídas `objects`, `table`, `report` (o objeto e as suas partes), `count` e `done`; sem nada a juntar devolve os mesmos objetos. Sem `image` a tabela é a do `regroup` (médias ponderadas pelas células); com `image`, refeita com o `stats_table`. Bateria `objects_fuse_bench` (os blocos 4 × 4 de um chessboard, a imagem de duas bandas; 1 e 8 threads, o mesmo hash): `alike` (`sd_b1 + sd_b2` no máximo 20, `improve` falso) 1000² em 0,17 s e 0,12 s (743 objetos), 2000² em 0,83 s e 0,88 s (3235); `compact` (compacidade no máximo 1,5, `mode` all; acaba em um objeto, a imagem inteira é um retângulo) 1000² em 0,07 s e 0,04 s, 2000² em 0,49 s e 0,27 s. Em 8 threads quase não ganha: o resto é serial (as somas das bandas são uma por banda, as junções são uma a uma). **Passadas incrementais (2026-11-20).** O perfil de `alike` 2000² em 1 thread (0,86 s): montar os pares 0,11, avaliar 0,12, propostas 0,05, as duas ordenações 0,24, junções 0,23, `State::new` 0,05, `regroup` e tabela 0,04; 86 passadas. Agora o valor de cada par fica na lista de vizinhos (NaN = a calcular), o valor próprio e as propostas de cada semente ficam guardados, e uma junção só deixa sujos o grupo novo e os seus vizinhos: só os pares e as sementes sujos são avaliados e propostos de novo (as propostas de uma semente limpa são reaproveitadas), sem ordenar por semente (o melhor candidato sai direto da varredura) e com as células da semente dentro da proposta (o comparador não toca a memória). As junções de uma passada, que são pares disjuntos, rodam em ordem de id (o resultado é o mesmo em qualquer ordem e a memória tocada fica perto: as junções caíram de 0,32 s para 0,17 s). A varredura dos pares sujos e as propostas rodam em paralelo. O resultado é bit a bit o mesmo (os quatro hashes de antes; teste `the_incremental_passes_join_as_a_full_evaluation_does` compara com uma reavaliação total a cada passada). O ganho em `alike` é pequeno porque as primeiras passadas juntam metade dos objetos e sujam quase tudo (1,4 milhão de grupos sujos em 86 passadas contra 0,25 milhão de objetos); o que sobra é a cauda de passadas e o custo por junção. Números (1 thread / 8 threads, antes → depois): `alike` 1000² 0,17 / 0,12 → 0,12 / 0,09 s, 2000² 0,83 / 0,88 → 0,61 / 0,49 s. O caso `compact` passou a ter um teto de tamanho no `fitting` (`compactness + cells / 20000`, `threshold` 1,5, `mode` best), para não acabar em um objeto só: 1000² 127 objetos (hash `1ab3f2c48690a8fa`), 2000² 491 (`f5fdda1f2ce28359`); antes (código antigo, mesmo caso) 0,07 / 0,04 s e 0,45 / 0,27 s, agora 0,05 / 0,04 s e 0,35 / 0,29 s. Testes: `crates/ops/tests/objects_fuse.rs`. **Retângulo girado (2026-11-20).** A caixa alinhada aos eixos dava compacidade mediana 1,72 em 1993 pedaços de copa contra 1,59 no `features`, e o `improve` com compacidade quase nunca disparava em pedaços de watershed. Agora cada grupo guarda o casco convexo dos cantos das suas células (`hulls_of`, a partir das corridas de cada linha, só quando o `fitting` usa comprimento, largura, compacidade ou índice de borda); o casco do juntado é o casco dos pontos dos dois cascos (cadeia monótona de Andrew sobre a união, `merge_hulls`) e o retângulo mínimo sai dos calibres rotativos do `features` (`hull_rect` chama o mesmo `min_rect`), então os números são os mesmos do `features` para o mesmo objeto (teste `the_joined_rectangle_is_the_one_objects_features_measures`, 1e-9, com células não quadradas). `improve` falso sem `threshold` agora avisa (`ctx.warn`) em vez de recusar: juntar todo par é o que alguns querem (refazer os grupos de uma classe), mas quase sempre é esquecimento. Bateria `objects_fuse_bench` (1 / 8 threads, antes → depois): `alike` não muda (hashes `1e7aaab11ec438c` e `b9e2ac30186df8aa`, sem casco); `compact` 1000² 0,06 / 0,04 → 0,13 / 0,07 s, 2000² 0,46 / 0,35 → 1,02 / 0,53 s; os hashes de `compact` (`1ab3f2c48690a8fa`, `f5fdda1f2ce28359`) NÃO mudam, porque as uniões de blocos de um chessboard têm o retângulo mínimo alinhado aos eixos (o caso não exercita a rotação; quem exercita é o teste dos Voronoi). Custo e a otimização (mesma noite): a primeira versão alocava e ordenava a união dos pontos a cada par (`compact` 2000² 1 thread 0,46 → 1,02 s). Agora os cascos são guardados com os vértices ordenados, o par junta as duas listas ordenadas num buffer por thread (`map_init`, `HullScratch`) e roda a cadeia de Andrew e os calibres sem alocar; um grupo que preenche a sua caixa (células = área da caixa) não guarda casco e o retângulo sai da caixa, com as mesmas contas do `min_rect` (bit a bit o mesmo). `compact` 2000² 1 / 8 threads: 0,56 / 0,45 s (hashes iguais aos de antes). A bateria ganhou `compact-blobs` (blobs de Voronoi crescidos até se tocarem, o `compact` com objetos que não são blocos alinhados aos eixos, que exercita a rotação): 1000² 0,02 / 0,01 s (193 objetos, hash `743376cba73bb355`), 2000² 0,16 / 0,09 s (797 objetos, hash `906ec0bc88cd4f38`).
  - Parâmetros: `seeds` e `candidates` (classes, padrão todas), `fitting` (uma expressão do `objects.calc` sobre o
    objeto juntado, por exemplo `compactness` ou `mean_nir`), `goal` (`max` ou `min`), `improve` (só junta se o
    juntado for melhor que a semente; padrão sim), `threshold` (opcional, o valor que o juntado precisa alcançar),
    `mode` (`best`, cada semente com o seu melhor candidato, o melhor mútuo como no multiresolution; ou `all`),
    `max_passes`. A classe do resultado é a da semente.
  - Os atributos do juntado sem refazer as features: as células, a área, o perímetro (pa + pb − 2 × a borda
    comum, de `neighbours_of`), a caixa (o comprimento e a largura aproximados), as médias e os desvios das bandas
    (somas e somas dos quadrados por objeto, da `image`), a compacidade e o índice de forma. As expressões são
    avaliadas em lote sobre uma tabela de pares, como o `pair_condition` do `objects.merge` por condição
    (`levels.rs` ~193). Por passada: os pares, o melhor por semente, os pares disjuntos juntados (como o `merge_all`),
    e de novo. Uma feature que não sai assim (textura, esqueleto, contexto) é recusada com a lista das que podem.
  - Testes: um prédio em L partido em três pedaços se junta pela compacidade; o `threshold` numa média; o mútuo; a
    recusa da textura; as passadas; 1 e 8 threads.
- **7. Templates e documentação (P). Feito (2026-11-20).** Os três templates em `templates.rs` (grupo dos objetos), cada um roda como está e tem teste em `the_object_templates_run`: "Classes by parcel" (`bounds` do multiresolution e `thematic` do `features`; o teste exige `th_share` 1 em todo objeto e os nove lotes), "Separate tree crowns" (limiar, `objects.split` por `surface` com `h` 0,3, `objects.fuse` por compacidade, `objects.extrema` do `max_image`; o teste exige mais cortes que manchas, menos objetos depois da fusão e topos que não são todos os objetos) e "Roads and rivers by the skeleton" (um mapa de cobertura, pedaços por classe com `segment.threshold`, `skeleton` e regras em `skel_length_width` e `skel_width`; o teste exige recall de 0,7 no rio e na estrada). O `objects.resize` não tem template próprio (a tabela do tutorial o cobre). O `from-ecognition.md` ganhou os `ops` e um item em "Going further". Plano original: No grupo dos objetos de `templates.rs`, com `data` nas entradas: "Classes por
  lote" (um vetor de lotes desenhado e uma imagem → multiresolution com `bounds` → features com `thematic` →
  classificação), "Copas de árvores separadas" (um nDSM de manchas do `nlm.*` → limiar → `objects.split` pelo
  `surface` → `objects.extrema` dos topos) e "Estradas e rios pelo esqueleto" (segmentar → `skeleton` →
  `skel_length_width` alto e `skel_width` baixo). O tutorial *from-ecognition* com as linhas novas. Testes: os
  templates rodam como estão e os de sempre de todos os templates.

**Decisões tomadas** (padrões; o usuário pode mudar):
1. O `bounds` é nativo no multiresolution, no region growing, no Felzenszwalb e no spectral difference, e um passo
   depois nos outros; uma feição do vetor é uma zona; as células fora de todas são uma zona.
2. Os operadores que mudam a geometria mantêm as classes e perdem os vínculos de nível, com um aviso.
3. O `objects.split` divide pela distância à borda; o `surface` é opcional.
4. Crescer e encolher sem condição são euclidianos exatos (pelo `edt`); com `mask` ou `into`, anel por anel.
5. O esqueleto é do raster (Zhang e Suen, com poda), não do polígono; a documentação diz que os valores diferem um
   pouco dos do eCognition.
6. O *find enclosed by class* fica na documentação, sem operador.
7. A fusão pelo resultado é um operador novo, `objects.fuse`, não um modo do `objects.merge` (que fica simples).

**Onde parou (2026-11-20).** Feitos os sete itens: 1 (as camadas temáticas: `065e46b`, `ec58391`), 2 (`objects.split`, `a2365f7`), 3 (`objects.resize`, `e175e9f`), 4 (o esqueleto), 5 (`objects.extrema`), 6 (`objects.fuse`) e 7 (os três templates, com o `from-ecognition.md`). O `website/data/ops.json` já os tem desde os dados do 1.59.0 (b29dae4); o build do site não avisa mais.

### Os tutoriais de OBIA, do básico ao complexo (pedido de 2026-11-19)

Hoje são três páginas: *Your first map by objects* (o template com a imagem sintética), e os guias de quem vem do
eCognition e do InterIMAGE; nenhuma tem figura. Falta uma série que ensine o método de verdade, com imagens reais e
figuras de execuções reais (`website/figures/object-based-image-analysis/<slug>.py`, como manda o
`website/AUTHORING.md`).

**Os dados.** Duas áreas, as mesmas na série inteira, para o leitor reconhecer a paisagem de uma página para a outra:
- **Sentinel-2** (10 m, as bandas azul, verde, vermelho, NIR e SWIR) de uma área com cidade, cerrado ou floresta,
  agricultura e água (a borda do Lago Paranoá, em Brasília, numa cena seca e sem nuvens de julho ou agosto), pelo
  `data.stac` do Earth Search (sem chave). A licença do Copernicus deixa redistribuir com atribuição.
- **NAIP** (0,6 m, RGB e NIR) de um bairro dos Estados Unidos, com casas, árvores, gramados, ruas e água, pelo
  `data.stac` do Planetary Computer (`sign = planetary_computer`). Domínio público.
- Um módulo comum, `website/figures/object-based-image-analysis/obia_data.py`, baixa e guarda os recortes (no
  `website/.cache`), e cada página traz o modelo com o nó `data.stac` (o leitor roda igual) e um recorte pequeno
  para baixar (em `website/static/data/tutorials/object-based-image-analysis/`, poucos MB), para quem estiver sem
  rede.

**A série** (`series: Object-based image analysis`, as partes em ordem), cada página no esboço do AUTHORING (a ideia,
quando usar, o que precisa, o modelo passo a passo, ler os resultados, os ajustes que mudam a resposta com um
`tfslider`, armadilhas, ir além, referências verificadas):
1. *Your first map by objects* (fica, iniciante): ganha figuras e aponta para a série.
2. **O que é um objeto: a segmentação** (iniciante): o que os segmentadores fazem, lado a lado na mesma imagem
   (multiresolution, SLIC, Felzenszwalb, watershed, threshold); a escala, a forma e a compacidade do multiresolution
   num `tfslider`; a segmentação ao vivo e a comparação no Viewer.
3. **A escala certa** (intermediário): a sub e a super-segmentação; o ESP (`segment.scale`) e o
   `segment.optimize` (a variância, o I de Moran, o gráfico e o *Apply*); a escala de cada classe.
4. **Descrever os objetos** (intermediário): os atributos espectrais, os índices (NDVI, NDWI), a forma, a textura
   (GLCM) e o contexto; o espaço de atributos no Viewer e a separabilidade.
5. **Classificar por amostras** (intermediário): `objects.samples` por clique, o vizinho mais próximo e a floresta
   aleatória, `objects.accuracy` (a matriz de erros, as áreas ajustadas); amostras autocorrelacionadas.
6. **Classificar por regras e por uma árvore de classes** (intermediário): `objects.rules` com pertinências fuzzy,
   a herança das classes, `objects.calc`; água, vegetação e área construída pelos índices.
7. **A água por limiares** (iniciante): `segment.threshold` (Otsu) no NDWI, `segment.spectral_difference`, os
   polígonos da margem; o template *Water by Otsu on the NDWI*.
8. **Níveis: de objetos finos a talhões e paisagens** (avançado): vários níveis (`from`), os atributos de pai e
   filhos, `objects.merge`, `objects.grow`, `objects.remove_small`, refinar por classe.
9. **Classes sem amostras: k-means e SOM** (intermediário): `ml.kmeans` e `ml.som` na tabela dos objetos,
   `objects.classify` sem amostras, dar nome aos grupos.
10. **Corrigir à mão** (intermediário): `objects.edit` pelo inspetor (juntar, cortar, apagar), as edições que
    sobrevivem a outra segmentação.
11. **A mudança por objetos** (avançado): duas datas do Sentinel-2, `objects.temporal` e
    `objects.transition_matrix` (o desmatamento ou a expansão urbana por objeto).
12. **Imagens grandes** (avançado): os tiles do multiresolution e do region growing, a estimativa do tempo e *Use
    tiles*, o cluster.
13. **Segmentação profunda: SAM e embeddings** (avançado): `segment.sam` (os pesos, os pontos) e `objects.embed`;
    quando valem a pena.
14. **Estudo de caso: a cobertura urbana pelo NAIP** (avançado): telhados, árvores, gramados, ruas e água, com
    níveis, regras e a acurácia; o mapa e os polígonos prontos para um relatório.
15. **Estudo de caso: o uso do solo de uma área protegida pelo Sentinel-2** (avançado): do dado ao mapa com a
    acurácia e as áreas, como num artigo.

Depois, com o bloco do eCognition: as camadas temáticas (classes por lote), as copas separadas (`objects.split`), a
morfologia (`objects.resize`), as estradas e os rios pelo esqueleto, os extremos locais e a fusão pelo resultado,
cada um com a sua página quando o operador sair.

**Como fazer.** Uma base primeiro (`obia_data.py`, os recortes, a parte 2), revisada; depois as outras em lotes de
duas a três páginas por agente, dois agentes por vez (as figuras rodam o `terraflux.exe` de release com no máximo 4
threads cada), cada um só nos seus arquivos (as regras de trabalho em paralelo do AUTHORING). Cada página: o build sem
avisos, o `--check` sem erros de grafo, as capturas lidas. Um commit por lote (`docs(tutorials): ...`).

**Onde parou (2026-11-20).** Feita a série inteira, 19 partes: as 15 acima e, do bloco do eCognition, 16 *Classes by
parcel*, 17 *Shaping objects* (`objects.resize`), 18 *Tree crowns* (`objects.split`, `objects.fuse`,
`objects.extrema`) e 19 *Roads and rivers by the skeleton*; todas no índice do app. Os tutoriais acharam e
corrigiram: `objects.grow` (a classe), os threads do ONNX, a coluna constante do `ml.*`, o estimador de área do
`objects.accuracy` (`design`), o `min_size` do `segment.felzenszwalb`, o `th_share`, a curvatura do esqueleto, o
retângulo do `objects.fuse` e as costuras dos tiles; as páginas foram refeitas depois das correções. Ressalvas: os
pontos de referência e de treino das partes 14 e 15 foram rotulados a olho, por um agente, sobre a própria imagem
(as páginas dizem isso): uma revisão humana antes de publicar. O item NAIP da série saiu do Planetary Computer
(`obia_data.naip()` usa a cópia publicada). Os avisos de operador desconhecido (`objects.fuse`, `objects.extrema`)
somem com o `ops.json` do próximo release.

**O que a base da série achou no `data.stac`** (2026-11-19), para um release próximo:
- **Feito (2026-11-20).** Ele lê só a banda 1 de cada asset (`-srcband 1`, `stac.rs` ~643). O NAIP tem RGB e NIR num asset só (`image`), e
  outros (as ortofotos, o Planet) também: um asset de várias bandas deve dar uma camada por banda, nomeadas pelos
  `eo:bands` do item (ou `image_1`...), ou um `bands` para escolher. Até lá, o NAIP da série é recortado pelo script
  e lido com `raster.read`.
- **Feito (2026-11-20).** A grade só vem em graus (`extent` e `cellsize` em graus) ou de um `like`. Um `crs` (o `extent` e o `cellsize` nesse
  CRS) daria a grade UTM direto, sem o `raster.reproject` depois (uma reamostragem a menos). Quando sair, os
  modelos da série passam a usá-lo.
  **Decisões (2026-11-20).** Camadas por banda: o nome é o `common_name` do `eo:bands` (ou do `raster:bands`), senão o `name`, senão `<asset>_<n>`; um asset de uma banda continua com o nome do asset e o mesmo cache (a chave só ganha `bands` quando ele é dado). O `bands` aceita nomes (qualquer caixa, `name` ou `common_name`), números a partir de 1 e `<asset>_<n>`, na ordem pedida, e só vale para os assets de várias bandas (uma banda que o asset não tem é erro, com a lista das que tem). O número de bandas vem do `eo:bands`/`raster:bands` do item; sem eles o arquivo é aberto (uma ida à rede por asset). Se dois assets dariam o mesmo nome de camada, as do asset de várias bandas ganham o nome do asset na frente. `crs` é validado pelo GDAL (`crs_wkt`), guardado como veio (`EPSG:32631`, sem o WKT, para não trocar a ordem dos eixos) e vale sem `like` (com `like` a grade é a do mapa: erro); o `cellsize` passa a ser obrigatório com `crs`. A busca no catálogo continua em graus (a caixa da grade reprojetada). Feito (2026-11-22), em parte: a grade UTM direto. A cadeia `data.stac` em graus + `raster.reproject` da série virou um só `data.stac` com `crs = "EPSG:31983"`, `extent` em metros e `cellsize = 10` (scripts `obia_data.py`, `obia_dates.py`, `large-images.py` e todos os modelos das páginas; a janela de 5,8 km, 580 x 580 células, tem os pixels um pouco diferentes, então as figuras, os CSV, os números e as amostras de treino e de referência das 14 páginas foram refeitos, e a `sentinel2_brasilia.tif` baixável também). Não feito: o NAIP com `crs` e `bands` nas três páginas que o leem (tree-crowns, roads-and-rivers, deep-segmentation): a cena da série (9 de novembro de 2024) saiu do Planetary Computer, e os itens que o catálogo tem sobre Plano são de 2020-10-14 e 2022-07-04 (o 2020 é o da parte 14, já com `crs` e `bands`); trocar a imagem muda as três páginas e as 6 execuções do SAM, então a série segue lendo o `naip_plano.tif` publicado, e isso está dito nas páginas. Também ficou para depois: o encoder de `objects.embed` da página deep-segmentation (precisa de `torchgeo` e `onnx`, que não estão instalados aqui), cujos números continuam os da execução antiga Conferido ao vivo: NAIP do Planetary Computer em UTM 14N a 10 m dá `red`, `green`, `blue`, `nir` (teste `planetary_computer_gives_the_bands_of_naip_in_utm`, `--ignored`).

### O OBIA mais completo: o que um especialista ainda sente falta (pedido de 2026-11-22)

A pergunta do usuário: o Terraflux deve ser o programa de OBIA mais completo e ao mesmo tempo o mais fácil; o que
falta? O fluxo central (segmentar ao vivo, comparar, ESP/USPO, amostras por clique, espaço dos atributos, regras,
aprendizado ativo, hipóteses, edição, acurácia por área, níveis, tiles, SAM) já passa o eCognition. Faltam cinco coisas
que aparecem em quase todo trabalho real e que um revisor de artigo pede. O usuário escolheu as cinco (2026-11-22);
ficaram de fora, por ora: a gestão de amostras (importar de vetor ou de um mapa antigo, atalhos 1–9, o painel de
equilíbrio), o relatório de um clique, o LiDAR direto (LAS/LAZ pelo PDAL) e o aviso da escala ao mudar de resolução.

Ordem, da mais rápida para a mais demorada (cada uma um commit, depois a próxima): 2, 1, 4, 5, 3. Começa depois das
sobras do OBIA em andamento (o bloco de 2026-11-22).

**Plano de implementação** (levantado no código em 2026-11-22). Valem para todos: cada operador novo com descrição,
portas, dicas (`tips.rs`/`advice.rs`), frases e referências no `methods.rs`, a lista "sem equivalente" do
`python_script` (o OBIA fica só no aplicativo), o guia `objects` do MCP (`guide.rs`), testes em
`crates/ops/tests/objects*.rs` e no `--test app`, o `CHANGELOG`, e o `website/data/ops.json` no próximo release.
Referências conferidas no Crossref antes de entrar no `methods.rs`.

- **1. A segmentação medida contra objetos de referência (M).** Hoje o `segment.optimize` (`objects/optimize.rs:218`)
  só mede sem referência: `score()` (`:48`, a variância interna ponderada e o I de Moran das médias), `objective()`
  (`:168`, soma ou F com `alpha`), a fronteira de Pareto (`:189-198`) e a tabela `scores` (`value`, `objects`,
  `variance`, `moran`, `objective`). Os artigos pedem as medidas supervisionadas, contra 20 a 50 polígonos desenhados.
  - Um operador `segment.evaluate`: entradas `objects` e `reference` (polígonos, ou um `Objects` vindo do
    `segment.from_vector`, `segment.rs:215-276`, que queima pelo `vector::burn`, `vector.rs:196`); o cruzamento é a
    tabela de sobreposição (referência × objeto, em células) de uma só varredura, como a do `thematic` em
    `features.rs:625`. Medidas (`metrics`, padrão todas): OS, US e D (Clinton et al. 2010, PE&RS 76(3):289-299; os
    objetos correspondentes pelo critério dele: mais de 50% da referência ou do objeto, ou o centróide dentro), ED2 com
    PSE e NSR (Liu et al. 2012, ISPRS J. 68:144-156), AFI (Lucieer e Stein 2002), QR (Weidner 2008) e o F das
    sobreposições (Zhang et al. 2015). Saídas: `table` (uma linha por referência: o objeto correspondente, a
    sobreposição, OS, US, D), `summary` (a média de cada medida e o ED2) e `objects` (a coluna `os`/`us` em cada
    objeto, para pintar no Viewer onde a segmentação erra por excesso ou por falta).
  - A referência bit a bit é o pacote R `segmetric` (Costa et al.; no CRAN, os nomes das medidas são os dele): os
    casos de teste em `tests/data/reference/segmetric.json`, gerados por um script em `tools/`, como os do PCA.
  - O `segment.optimize` ganha a entrada `reference` e os objetivos `d`, `ed2` e `f_overlap`: cada candidato é medido
    também por `segment.evaluate`, e a tabela `scores` ganha as colunas; sem `reference`, nada muda. O gráfico do
    optimize (`charts.rs::scores`, `optimize_view.rs`) mostra a medida escolhida e o clique escolhe o valor (a sobra
    do bloco de 2026-11-22).
  - **Feito (2026-11-22), a parte dos operadores**: `segment.evaluate` (`objects/evaluate.rs`) e a `reference` do `segment.optimize`
    (objetivos `d`, `ed2`, `f_overlap`; a tabela `scores` ganha `os`, `us`, `d`, `ed2`, `f_overlap`, a coluna `objective` é a medida
    escolhida de 1, a melhor, a 0, e o gráfico acha as colunas pela posição das cinco primeiras). Igual ao `segmetric` 0.3.0 (R 4.4.2) a 1e-9
    nas medidas e nos pares (`tools/make_segmetric_reference.R`, `tests/data/reference/segmetric/`: 5 casos, a referência e a segmentação
    como polígonos de células inteiras e como rótulos). **O `segmetric` 0.3.0 não tem PSE, NSR nem ED2 de Liu et al. 2012** (só o ED3 de Yang
    et al.): eles seguem o artigo como li (os objetos correspondentes a uma referência são os com mais da metade das células nela, ou o de maior
    sobreposição; PSE as células deles fora da referência sobre as da referência; NSR |m - v| / m; ED2 a raiz da soma dos quadrados), sem
    referência para conferir. O Weidner 2008 não tem DOI no Crossref (os dados do artigo vêm de memória). Nas propriedades, "cortar cada referência
    em dois sobe o US e não o OS" vale quando o que se corta é a referência (cada objeto passa a ter duas); cortar a segmentação sobe o OS.
  - **Feito (2026-11-22), a parte do aplicativo**: `reference_link.rs`. Em *Draw on the map*, com objetos no Viewer e polígonos desenhados, o botão
    *Add as reference objects* grava o GeoPackage (`drawn/`) e faz, num só passo de desfazer, um `vector.read` e um `segment.evaluate` dos
    objetos mostrados (se já há um `segment.evaluate` desses objetos, o leitor é trocado); o painel *Reference objects* mostra ED2, D, F, OS e US,
    a caixa *Show where it errs* acende uma camada divergente de `os - us` por objeto (vermelho: a referência foi cortada; azul: o objeto engoliu o que
    está fora) com legenda, e oferece *Use as the reference of <optimize>* para cada `segment.optimize` sem referência (um passo de desfazer). Template
    "Choose the segmentation by reference objects" (a `nlm.image` com as regiões verdadeiras, um polígono para cada quarta região, `segment.optimize` de 2 a 40
    de 2 em 2 com o objetivo `ed2`: escolhe 14, com 8 objetos para 15 regiões, e o ED2 mínimo). Parte 20 da série de tutoriais, "Measuring a segmentation
    against reference objects", com doze polígonos desenhados à mão sobre o recorte do Sentinel-2: o ED2 escolhe a escala 6 (313 objetos), o D mínimo fica em 8 e o
    USPO sem referência em 1,5. As figuras do `website/figures/object-based-image-analysis/measuring-against-reference-objects.py`, com o `terraflux` de release do worktree.
    Os avisos `unknown operator [[segment.evaluate]]` do `build.py --check` somem quando o `website/data/ops.json` for refeito no release.
  - No aplicativo, como estava planejado: na seção dos objetos, *Draw reference objects* desenha polígonos com a ferramenta de desenho do
    Viewer (os atributos do desenho já ficam no modelo) e liga ao `segment.evaluate` num passo de desfazer; a
    camada `os`/`us` com a rampa divergente (vermelho onde a referência foi cortada, azul onde foi engolida).
  - Template "Choose the segmentation by reference objects" (a `nlm.image` com polígonos de referência feitos dos
    rótulos verdadeiros do `nlm`; o teste exige que o ED2 escolha uma escala perto da verdadeira) e uma parte nova da
    série de tutoriais.
  - Testes: o `segmetric` bit a bit; propriedades (segmentação igual à referência dá OS = US = 0; cortar cada
    referência em dois sobe o US e não o OS); o `optimize` com referência escolhe o valor do menor ED2.

- **2. A validação cruzada espacial e a matriz de confusão (M).** O `ml.fit` (`ml/mod.rs` ~355-389: `column`,
  `columns`, `task`, `sample`, `stratify`, `seed`) não tem validação nenhuma, nem OOB; o `sdm.fit` tem (`fold`, `cv`,
  `workflow::fit`, `sdm/mod.rs:1268-1390`), e os geradores de folds gravam uma coluna `fold` (`with_folds`,
  `sdm/mod.rs:904-926`; `sdm/folds.rs`: `spatial_blocks` `:522`, `buffer_folds` `:728`). Objetos vizinhos são quase
  iguais, então a acurácia por amostras sorteadas sai otimista (Ploton et al. 2020; Meyer e Pebesma 2022).
  - Um operador `objects.folds`: entrada `objects` e `samples` (a tabela do `objects.samples`); `method` `blocks`
    (blocos de `size` em unidades do mapa, pelos centróides), `clusters` (k-means dos centróides, como o
    `sdm.spatial_clusters`), `neighbours` (um objeto e os seus vizinhos até `steps` passos nunca caem em folds
    diferentes: os componentes do grafo de vizinhança restrito às amostras, distribuídos pelos folds com o
    equilíbrio das classes) e `random`; `k` (padrão 5), `seed`. Reaproveita o `sdm/folds.rs` onde der (os centróides
    como pontos). Saída: a tabela com a coluna `fold`.
  - O `ml.fit` ganha `fold` (o nome da coluna; sem ela e com `cv = k`, folds sorteados e estratificados) e a saída
    `cv`: por fold e no total, a exatidão, o kappa, o F1 por classe e a matriz somada (regressão: RMSE, MAE, R²); o
    modelo final é ajustado em todas as amostras, como hoje. Com a floresta aleatória, o OOB (`ml/forest.rs`: guardar
    as linhas fora de cada árvore) como coluna a mais, avisando que o OOB não é espacial.
  - O `objects.accuracy` (`accuracy.rs:84-158`) ganha a entrada opcional `training` (a tabela das amostras de
    treino): conta as referências que são também de treino ou encostam num objeto de treino (pela vizinhança dos
    objetos), diz no `overall` e avisa; o aviso do `advice.rs:488-503` (o mesmo `objects.samples`) continua.
  - No aplicativo: a saída `matrix` do `objects.accuracy` e a `cv` do `ml.fit` viram um gráfico de matriz de confusão
    (`charts.rs`, um `Kind::Confusion`: as contagens com a cor pela proporção da linha, as exatidões do usuário e do
    produtor nas margens, a exatidão global no título). Um clique numa casa acende no Viewer os objetos daquela classe
    do mapa com aquela referência (`confusion_link.rs`, como o `som_link.rs`: a lista `errors` já tem os ids) e a
    lista leva a cada um, como os erros no `feature_space.rs:748-753`.
  - **Feito (2026-11-22), a parte dos operadores**: `objects.folds` (`objects/folds.rs`: `neighbours` com os componentes do grafo
    de vizinhança até `steps`, distribuídos do maior para o menor no fold que mantém as classes mais parelhas, a ordem
    entre iguais pela `seed`; `blocks` e `clusters` pelo `sdm/folds.rs`, sobre os centróides; `random` estratificado), o `fold`
    e o `cv` do `ml.fit` (`ml/cv.rs`; saídas `cv` e `cv_matrix`, a mesma tabela que o `sklearn` com os mesmos folds, a 1e-12, para a
    floresta de 10 árvores), o `training` do `objects.accuracy` (os objetos de referência que são de treino ou encostam
    num; entra no `overall` e avisa) e o template "Classify an image by clicking samples" (`objects.folds` mais duas
    `ml.fit`, uma por lugar e uma sorteada). O OOB da floresta ficou de fora: `forest::grow` não guarda as linhas de
    cada bootstrap; seria uma mudança no `Forest`. O gráfico da matriz de confusão e a ligação com o Viewer: **Feito (2026-11-22)**
    (`Kind::Confusion` em `charts.rs`, reconhecido pela tabela `map` mais uma coluna por classe de referência, para a `matrix` do
    `objects.accuracy` e a `cv_matrix` do `ml.fit`: as contagens com a cor pela parte da linha, a exatidão do usuário e do
    produtor nas margens, a global no título; `confusion_link.rs`: a casa clicada acende no Viewer os objetos de referência
    daquele par sobre os objetos classificados, e a lista leva a cada um. O `objects.accuracy` ganhou a saída `sampled`
    (id, mapa, referência de todo objeto de referência) para a diagonal, que o `errors` não tem). O item 2 está pronto.
  - Template: o "Classify an image by clicking samples" ganha o `objects.folds` (`neighbours`) e a `cv` do `ml.fit`,
    com uma nota sobre a diferença entre a acurácia sorteada e a espacial. Testes: folds sem vizinhos separados;
    a `cv` igual à do `sklearn.model_selection.cross_val_score` com os mesmos folds para a árvore de decisão
    determinística; o clique na casa acende os ids certos.

- **4. Levar o modelo para outra cena (M).** O `ml.predict` e o `objects.classify` não dizem quando um objeto está
  fora do que o treino viu. O `sdm.mess`, o `sdm.mop` e o `sdm.exdet` (`sdm/extrapolation.rs`: `mess` `:78`, `mop`
  `:208`, `exdet` `:345`) já aceitam tabelas pelo `frame_of` (`sdm/mod.rs:74-97`) e devolvem tabelas pelo
  `frame_out`.
  - O modelo do `ml.fit` passa a guardar o resumo do treino de cada preditor (mínimo, máximo, quantis 1/25/50/75/99,
    média e desvio) e, só quando pequena, a própria tabela de treino (para o MOP; acima de 10 000 linhas, uma amostra).
  - O `objects.classify` e o `ml.predict` ganham `novelty` (`mess` padrão, `mop`, `none`): as colunas `mess` (ou
    `mop`) e `novel` (verdadeiro fora da faixa) por objeto, e um `ctx.warn` com a parte dos objetos e da área fora,
    e quais preditores mais pesam (o `exdet`). No Viewer, a camada dos objetos novos hachurada sobre a classificação.
  - A diferença radiométrica: o modelo guarda os quantis de cada banda da imagem do treino; o `objects.classify`
    sobre uma imagem cujas medianas ou amplitudes diferem mais que um limiar avisa ("a cena é mais clara que a do
    treino") e aponta o operador novo `raster.match_histogram` (a correspondência de histogramas por banda, com
    `mask` para as áreas comuns; uma normalização relativa simples, sem o IR-MAD de Canty et al. 2004, que fica para
    depois se pedido). O aviso de bandas em escalas diferentes (`objects/warnings.rs:34-64`) continua.
  - Conferir antes de começar: se um modelo ajustado pode ser gravado e lido em outro modelo (um `ml.save`/`ml.load`
    ou o `model` gravado num arquivo); se não, entra aqui (o modelo é JSON com os parâmetros e o resumo do treino).
  - Template "Classify a second scene with the model of the first" (duas `nlm.image`, a segunda mais clara e com uma
    classe que a primeira não tem: o teste exige o aviso, os objetos novos na classe que faltava e a acurácia melhor
    depois do `raster.match_histogram`). Testes: o MESS dos objetos igual ao do `sdm.mess` sobre a mesma tabela;
    histograma casado com os quantis iguais aos da referência.
  - **Feito (2026-11-22).** O modelo do `ml.fit` guarda `training` (o resumo de cada preditor, min, max, quantis
    1/25/50/75/99, média e desvio, e as linhas do treino, uma amostra sorteada de 10 000 acima disso; `ml/novelty.rs`),
    com `serde(default)`: o JSON de um modelo antigo, sem ele, lê e não dá novidade. Não havia como levar um modelo para
    outro grafo (só a saída `[outputs]` o gravava como JSON): entram `ml.write` e `ml.read` (`ml/file.rs`; o `ml.read`
    tem a entrada `after` para ordenar depois do `ml.write` no mesmo grafo). O `objects.classify` e o `ml.predict`
    ganham `novelty` (`mess` padrão, `mop`, `none`): as colunas `mess` (ou `mop`) e `novel` na tabela e nos atributos
    dos objetos (no `ml.predict` com mapas, a saída `novelty`), o `sdm::extrapolation::mess` e o `mop` sobre as linhas
    guardadas, e um aviso com a parte dos objetos e da área fora (a partir de 5 %) e os preditores que mais pesam (o
    `exdet`, ou a contagem fora da faixa quando a covariância é singular). O aviso da diferença radiométrica não
    precisou da imagem do treino: o modelo não a conhece, então vale os preditores `mean_*`/`median_*` (as médias das
    bandas) contra a mediana e o IQR do treino, a partir de 0,75 IQR em metade deles do mesmo lado; ele aponta o novo
    `raster.match_histogram` (`raster_match.rs`: a definição do `skimage.exposure.match_histograms`, os quantis da
    origem sobre os da referência por interpolação, e a `mask` para a área comum; igual ao scikit-image, o teste lê
    `tests/data/reference/match_histogram.json` de `tools/make_match_histogram_reference.py`). No Viewer os objetos
    `novel` aparecem listrados (`objects_view.rs`: listras de duas células sobre a classificação, o resto do mapa
    aparece por baixo). O template "Classify a second scene with the model of the first" (a segunda cena mais clara
    e com a classe urbana que a primeira não tem; o modelo vai e volta pelo arquivo): o teste exige os dois avisos, os
    objetos novos na classe que faltava (urbano 100 % contra 39 % dos outros) e a acurácia no chão comum de 0,46 para
    0,93 depois do `raster.match_histogram`. Duas coisas que a medida mostrou: o MESS com muitos preditores e poucas
    amostras marca muito objeto normal (57 % dos da própria cena de treino com os 20 atributos do `features`; 15 % só
    com as médias das bandas e o NDVI), por isso o template ajusta o modelo nessas colunas e o aviso só vem a partir de
    5 %; e o `raster.match_histogram` casa o histograma, não o objeto: o segmentador roda de novo sobre a cena casada.
    Ficou de fora: a AOA de Meyer e Pebesma (2021) e o IR-MAD de Canty et al. (2004).

- **5. As séries temporais por objeto (M/G).** Hoje o `objects.features` (`features.rs`) dá média, desvio, mínimo,
  máximo e mediana por camada (`per_band` `:713`) e os índices das bandas nomeadas, sem eixo do tempo; o
  `objects.temporal` (`temporal.rs`) é a matriz de transição de duas datas (Feitosa et al. 2009), não uma série; não
  há fenologia em lugar nenhum do código; o inspetor (`objects_inspect.rs:286`) não tem gráfico. O uso mais comum do
  OBIA hoje é mapear lavouras pela série do Sentinel-2.
  - Um operador `objects.series`: entradas `objects` e `series` (uma pilha, uma camada por data, de um índice ou de
    uma banda; ou várias pilhas, uma por variável); as datas pelos nomes das camadas (`YYYY-MM-DD` ou `YYYYMMDD` em
    qualquer parte do nome, como o `data.stac` as escreve; conferir) ou pelo `dates`. Por objeto e data, a média das
    células com valor (NaN de nuvem fica de fora; a data inteira sai NaN quando menos de `min_share` das células têm
    valor). Saídas: `objects` com as colunas (grupos `values`: `<var>_<data>`; `phenology`; `harmonics`) e a tabela
    longa (`id`, `date`, `value`) para o gráfico.
  - `phenology`: o máximo, o mínimo, a amplitude, o dia do pico, a integral (trapézios no tempo), o início e o fim da
    estação pelo limiar de uma fração da amplitude (`season_threshold`, padrão 0,5; o método do TIMESAT, Jönsson e
    Eklundh 2004), a duração, as taxas de subida e de descida; sobre a série alisada (`smooth` `none`, `savgol` de
    Savitzky e Golay com janela e grau, ou `harmonic`), os buracos de nuvem interpolados só para essas medidas.
  - `harmonics`: o ajuste por mínimos quadrados de `n` harmônicos (padrão 2) com o período de um ano (Jakubauskas et
    al. 2001; o mesmo modelo do CCDC sem as quebras): a média, a tendência opcional, a amplitude e a fase de cada
    harmônico e o RMSE; poucos valores (menos de `2n + 2`) dão NaN.
  - Os mesmos núcleos para mapas (um `raster.phenology` por célula, em paralelo por linhas) se não custar mais que
    expor as funções: os objetos e as células com as mesmas contas.
  - **Feito (2026-11-22), a parte dos operadores**: `objects.series` (`objects/series.rs`; grupos `values`, `phenology`, `harmonics`,
    a tabela longa `series` com `id`, `date`, `doy`, `value`, `smoothed`) e o `raster.phenology` (os mesmos núcleos por célula, em `phenology.rs`:
    custou pouco). As datas pelos nomes das camadas (`YYYY-MM-DD`, `YYYY_MM_DD`, `YYYY.MM.DD` ou `YYYYMMDD` em qualquer parte) ou pelo `dates`; a pilha
    não carrega datas e o `data.stac` dá compostos (uma camada por ativo), não séries; recusa nomes sem data, datas que não crescem, menos de 4 datas.
    Fenologia como o método da amplitude do TIMESAT: cada lado do pico parte do seu próprio mínimo (Jönsson e Eklundh 2004): o nível do início é `esq + t (max - esq)` e o do fim
    `dir + t (max - dir)`; o início e o fim são onde a série cruza o seu nível andando do pico, por interpolação linear (sem cruzamento, NaN); a integral por
    trapézios em dias; taxas (max - nível do lado) sobre os dias do início ao pico e do pico ao fim. Sem colunas `base_left`/`base_right`. Savitzky-Golay igual ao `scipy.signal.savgol_filter(mode="interp")` a 1e-9 numa série de
    datas regulares, sobre a série de todas as datas com os buracos preenchidos por retas no tempo; com datas irregulares ele as lê como regulares e um
    aviso diz (a janela é de datas, não de dias). Harmônicos iguais ao `numpy.linalg.lstsq` a 1e-9 (`tools/make_phenology_reference.py`,
    `tests/data/reference/phenology.json`); o tempo é em dias desde 1 de janeiro do ano da primeira data, a fase em [0, 2 pi), a tendência por ano, o RMSE sobre
    os valores. Várias pilhas por variável não: uma pilha por chamada, o `variable` dá o prefixo das colunas, e `objects.join` junta as de outras chamadas.
  - No aplicativo: o inspetor ganha o perfil do objeto escolhido (a série da tabela longa, os pontos de nuvem como
    buracos) com a faixa das amostras da classe (mediana e intervalo interquartil por data) e um segundo objeto
    para comparar (Ctrl+clique); no espaço dos atributos, as colunas da fenologia como quaisquer outras.
  - **Feito (2026-11-22), a parte do aplicativo**: `series_view.rs`. O inspetor desenha o perfil do objeto escolhido quando um `objects.series` fez os
    objetos (acha o nó pelos rótulos compartilhados e lê a tabela longa `series`): os valores por data como pontos (a nuvem é buraco), a linha
    suavizada, a faixa das amostras da classe do objeto (mediana e quartis por data, dos objetos marcados do `objects.samples`) e, com Ctrl+clique, um
    segundo objeto para comparar. Template "Crops by their seasons" (parcelas de `nlm.mosaic_gibbs` em quatro culturas, 24 datas de NDVI por classe
    com campo que desloca a estação, ruído e nuvens; `objects.series` com fenologia e harmônicos, floresta aleatória sobre essas colunas e `cells`:
    pureza 1,00 e as três maiores importâncias são colunas da estação). Parte 21 da série de tutoriais, "Seasons of objects", com 23 janelas de 16 dias do Sentinel-2
    sobre lavouras de Sorriso, MT (uma `data.stac` por janela com `mask = auto`, a `data.stac` só compõe, não dá séries).
  - Template "Crops by their seasons" (objetos de lotes por `nlm.*`, cada classe com a sua curva de NDVI, ruído e
    datas de nuvem sorteadas; amostras, floresta aleatória sobre a fenologia e os harmônicos; o teste exige a pureza)
    e uma parte nova da série de tutoriais com uma série real do Sentinel-2 pelo `data.stac`.
  - Testes: curvas analíticas (um seno dá a amplitude, a fase e o pico exatos; o trapézio de uma reta); o Savitzky e
    Golay igual ao `scipy.signal.savgol_filter`; os harmônicos iguais ao `numpy.linalg.lstsq`; NaN nas datas de
    nuvem; 1 e 8 threads com o mesmo resultado; o perfil no inspetor no `--test app`.

- **3. O SAM interativo e os objetos parecidos (G).** O `segment.sam` (`objects/deep.rs`) só tem a grade automática
  (`SamParams` `:290-308`; `sam_tile` `:320-`: o codificador uma vez por tile, o decodificador em cada ponto, com um
  ponto positivo e um de preenchimento, `:359-369`), mas o decodificador aceita quaisquer pontos e caixas (é um tensor
  de coordenadas e rótulos). O `sam_view.rs` é só a janela dos pesos; não há ferramenta de clique para o SAM. O
  `objects.embed` (`deep.rs:767-832`) dá um vetor por objeto.
  - O `segment.sam` ganha a entrada `prompts` (um vetor de pontos com a coluna `label`, 1 dentro e 0 fora, e
    caixas como polígonos, agrupados por `group`; opcional `class`): um objeto por grupo, o codificador só nos tiles
    que os prompts tocam; sem `prompts`, a grade de hoje. O modelo guarda os prompts, não as máscaras: reproduzível e
    pequeno, como as amostras.
  - No Viewer, a ferramenta *SAM* na seção dos objetos: o codificador da janela visível roda num worker (uma vez por
    janela, de 1 a 3 s na CPU; a barra de estado diz), e cada clique roda só o decodificador (dezenas de ms) e mostra a
    máscara ao vivo; botão esquerdo dentro, direito fora, arrastar uma caixa; Enter aceita (grava o prompt no vetor do
    nó num passo de desfazer), Esc descarta. Com uma classe escolhida, o objeto aceito vira também uma amostra.
  - Patentes (`docs/patents-objects-resolve.md`): os objetos do SAM ficam como um `Objects` próprio; nada de
    "imprimir" uma segmentação na outra. Juntar com outra segmentação é pela edição manual que já existe
    (`objects.edit`), feita pela pessoa; conferir o texto antes de oferecer qualquer atalho que combine as duas.
  - Os objetos parecidos: um operador `objects.similar` (entradas `objects`; `examples`, os ids ou uma tabela de
    amostras; `columns` padrão os `emb_*` do `objects.embed` quando existem, senão os atributos padronizados;
    `metric` `cosine` ou `euclidean`; `top` ou `threshold`): a coluna `similarity` e a seleção. No inspetor, *Find
    similar*: um controle do limiar acende os objetos no mapa ao vivo e *Label all as <classe>* grava as amostras num
    passo de desfazer, o rotular de muitos com poucos cliques que o eCognition não tem.
  - Testes: o decodificador com prompts igual ao do SAM de referência em Python (o `segment-anything` com os pesos
    quantizados, uma vez, guardado em `tests/data/reference`); um prompt fora de um objeto não o inclui; os tiles só
    onde há prompts; o `objects.similar` cosseno igual ao do numpy; no `--test app`, um clique com o decodificador
    falso (sem os pesos) grava o prompt e o desfazer o tira.
  - **Feito (2026-11-22), passo A: os objetos parecidos.** `objects.similar` (`objects/similar.rs`): a similaridade de
    cada objeto ao exemplo mais próximo (o melhor deles), pelo cosseno ou por 1 / (1 + distância euclidiana); os
    exemplos são os `examples` (ids), a tabela `samples` (coluna `id`) ou, sem nenhum, os objetos que já têm classe; as
    colunas são as dadas, senão as `emb_*` quando há, senão os atributos numéricos padronizados (sem id, cells, area;
    a saída `used` diz quais; uma coluna de um valor só sai); a seleção é os `top` (20 sem limiar) ou os acima do
    `threshold`, sem os exemplos, do mais parecido para o menos. A função (`similarity`, `above`, `top`) é a que o app
    usa. No inspetor (`objects_inspect.rs`), *Find similar*: o cosseno ou o euclidiano e o limiar, que acende ao vivo
    no mapa os objetos parecidos com os selecionados (a seleção tem a união; o resultado fica guardado enquanto a
    seleção, os objetos e a métrica não mudam, só o limiar roda a cada quadro) e *Label all as <classe>*, que dá a
    classe como amostra aos acesos e aos selecionados e escreve no modelo num passo de desfazer pelo mesmo caminho do
    *Apply* (`write_now`). Testes: o cosseno e o euclidiano iguais ao numpy nas embeddings e nos atributos
    padronizados (`tools/make_similar_reference.py`, `tests/data/reference/similar.json`); `top` e `threshold`;
    no `--test app` o clique seleciona, o limiar acende o mesmo que o operador acharia e *Label all* escreve N amostras
    e um desfazer as tira. Faltam os passos B e C.
  - **Feito (2026-11-22), passo B: os prompts do `segment.sam`.** A entrada `prompts` (um vetor, em qualquer sistema de
    coordenadas): um ponto com `label` 1 (o padrão) é dentro do objeto e com 0 fora; um polígono é uma caixa (o seu
    retângulo); `group` diz de que objeto cada feição é (sem a coluna, cada feição é um objeto) e `class` dá a classe
    do objeto. Um objeto por grupo: o decodificador recebe os pontos (1 e 0) mais o ponto de preenchimento (-1), ou,
    com caixa, os dois cantos como rótulos 2 e 3 e sem o preenchimento; vale a máscara de melhor IoU, cortada nas
    células com valor e reduzida à área conexa que tem o primeiro ponto (ou o meio da caixa; senão a maior). O
    codificador roda uma vez nos tiles de 1024 onde um grupo tem o primeiro ponto, e só neles; onde dois objetos se
    cobrem fica o menor; `fill` faz do resto o objeto do grupo 0; um grupo sem máscara (só pontos fora, fora da
    imagem, máscara vazia) sai num aviso. A tabela ganha a coluna `group`, e as classes dos grupos viram as `classes`
    dos objetos (e dão as amostras de `objects.features`). Sem `prompts`, a grade de hoje. Peças públicas que o passo
    C usa: `Prompt`, `Group`, `Embedded`, `encode_tile` (o codificador de um tile, guardado) e `decode_group` (só o
    decodificador). Testes: as máscaras de pontos dentro, de ponto fora, de caixa e de caixa com ponto no ONNX Runtime
    do Python com os pesos quantizados (`tools/make_sam_prompts_reference.py`, `tests/data/reference/sam_prompts.json`;
    o teste `prompts_match_onnxruntime_in_python` é `#[ignore]` e pula sem o `ORT_DYLIB_PATH` e os pesos): as quatro
    máscaras com IoU 1,0000 (as mesmas células); com um decodificador falso, os tensores dos cliques, um objeto por
    grupo, o ponto fora tirando as células, o codificador só nos 2 de 6 tiles tocados, os grupos sem objeto avisados e
    a leitura do vetor. Falta o passo C.
  - **Feito (2026-11-23), passo C: a ferramenta SAM do Viewer** (`sam_tool.rs`). Na seção dos objetos, *SAM tool*
    (desliga as amostras): o codificador roda uma vez na parte em vista com a margem de `live_segment::window_cells`, numa
    thread (a barra de estado diz, e o painel mostra o spinner e *ready*); de novo só quando a vista sai da janela ou
    entra tanto que a janela tem menos de um terço dela. Cada clique roda só o decodificador (outra thread; o último
    pedido vale) e a máscara aparece ao vivo: esquerdo dentro, direito fora (pontos verdes e vermelhos), arrastar uma
    caixa (a segunda substitui a primeira; o botão do meio move o mapa; nenhum clique escolhe célula ou objeto
    enquanto a ferramenta está ligada). Enter (ou *Keep*) grava o grupo no modelo num passo de desfazer: um
    GeoPackage em `drawn/` ao lado do modelo, com nome do conteúdo (o desfazer encontra o arquivo que tinha; sem
    caminho do modelo, a pasta do cache), lido por um `vector.read` (`sam_prompts`) que alimenta o `prompts` de um
    `segment.sam` (`sam`) da mesma imagem; os grupos seguintes entram no mesmo arquivo e no mesmo nó. Esc solta o
    prompt. Com uma classe escolhida (a paleta das amostras) o grupo leva a `class`, e é por ela que o objeto é amostra
    (o `segment.sam` do passo B dá as `classes` aos objetos). Os objetos do SAM ficam do nó dele: nada é impresso em
    outra segmentação. Limites: a imagem é a `image` do nó dos objetos mostrados (os três primeiros layers, ou os
    `bands` do nó) e o mapa mostrado tem de estar na grade dela; os pesos têm de estar no cache (senão a mensagem manda
    rodar um `segment.sam` uma vez, que pergunta antes de baixar). `Session` do ONNX ganhou `Send + Sync` (a sessão do
    ONNX Runtime serve a várias threads). Testes no `--test app` (`objects_sam.rs`) com um modelo falso: o codificador
    uma vez e um decodificador por clique, ponto fora, caixa, Esc, Enter e o desfazer em dois passos; e, com `--ignored`,
    o modelo de verdade pelo mesmo `Backend` (IoU 1,000 num quadrado) e as figuras nos temas Slate e Paper.

## PCA, k-means e SOM para mapas, pilhas e tabelas

Pedido de 2026-11-19: operadores de componentes principais (PCA), k-means e mapas auto-organizáveis (SOM), com o
código copiado ou baseado no do Janus e do Zeit onde fizer sentido. Por ora só o plano; quando for implementado, um
commit por etapa, na ordem.

**O que já existe.** No Terraflux, `crates/ops/src/signatures.rs` já tem PCA (`svd` da matriz dos z-scores, sinal de
`sign_of`), k-means (k-means++ pelo `NumpyRng`, Lloyd até 300 iterações, o melhor de `n_init`), NMF e as ligações do
scipy, iguais ao landschaft até 1e-11. Mas só para as tabelas de `metrics.signatures` (`metrics.pca`,
`metrics.cluster`, `metrics.clustergram`): o SVD da matriz inteira (`Vec<Vec<f64>>`) serve para centenas de linhas,
não para os milhões de células de uma imagem. Os gráficos do círculo de correlações e do clustergram (`charts.rs`) já
leem essas tabelas. O k-means dos folds espaciais (`sdm/folds.rs`) é o do R pela ponte C++, e fica como está.
Fora daqui:
- **Janus** (`crates/janus-zeit/src/cluster.rs`): k-means com o k-means++ guloso do scikit-learn (2 + ln k
  candidatos), somas por blocos fixos com rayon (o mesmo resultado com qualquer número de threads), parada em 300
  iterações ou tol × variância, e Ward; só depende do rayon. PCA por covariância e Jacobi cíclico em
  `crates/janus-app/src/atlas_math.rs` (boa para poucas bandas).
- **Zeit** (`rust/zeit-core/src/som.rs`): o SOM portado operação a operação do MiniSom 2.3 (os mesmos bits): grade
  retangular, vizinhanças gaussiana, bubble e triangle, as funções de decaimento, `train_online`, `train_batch` em
  paralelo e `predict_bmus`. Depende do módulo `parallel` do zeit-core, que é trocado pelo rayon. Não tem grade
  hexagonal nem U-matrix. O PCA do Zeit (`improphe.rs`) usa faer e está preso ao IMPROPHE: não é copiado.
Os três projetos são GPL-3.0, do mesmo autor; o Terraflux é GPL-3.0-or-later: nada impede a cópia.

**Princípios.**
- **A família `ml.*`**, ao lado de `ml.fit`: `ml.pca`, `ml.kmeans`, `ml.som`. Como o `ml.fit`, cada um lê uma
  tabela (as linhas, as colunas escolhidas) **ou** uma pilha (uma camada por variável, com `cells` como máscara), e dá
  as saídas diretas (o mapa ou a coluna) e um `MlModel` para aplicar a outros dados.
- **Ajustar numa amostra, aplicar em tudo.** O ajuste é `Access::Global` sobre uma amostra das células (o `sample`
  do `ml.fit`, sorteada pelo `NumpyRng`; o padrão sendo todas até um limite e uma amostra acima dele, dito na nota); a
  aplicação, pelo `ml.predict`, é `Access::Local`, por blocos, e roda nos mapas maiores que a memória
  (`crates/engine`) e no cluster. O modelo guarda a média e o desvio do ajuste: aplicado à imagem de outra data, usa
  os do ajuste, não os da nova.
- **O mesmo resultado com 1 ou 8 threads** (somas por blocos fixos, como o Janus), e a mesma semente dá os mesmos
  mapas. Uma célula com NaN em qualquer variável fica fora do ajuste e sai NaN.
- **Padrões para quem está começando:** padronizar quando as variáveis estão em unidades diferentes (o padrão, e um
  aviso do check quando desligado nesse caso), o número de componentes pela variância explicada (95%), o k escolhido
  pela silhueta quando não dado, e o tamanho da grade do SOM pela regra de Vesanto (5 √n nós), cada escolha automática
  dita na nota do nó.
- **Um módulo só**: `crates/ops/src/multivar/` (`linalg.rs`, `pca.rs`, `kmeans.rs`, `som.rs`). O `signatures.rs`
  passa a chamá-lo, e o `metrics.pca` e o `metrics.cluster` dão exatamente as mesmas tabelas de hoje (os testes de
  paridade com o landschaft continuam passando sem mudança).

**As etapas.**

- **A. O módulo comum.** `multivar/linalg.rs`: o `svd` que hoje está no `signatures.rs` (para tabelas), a covariância
  e a correlação por blocos em uma passada (a soma de Kahan ou por pares, em paralelo por faixas de linhas) e o Jacobi
  cíclico do Janus para a matriz d × d (d = bandas, poucas dezenas). `multivar/kmeans.rs`: o k-means do Janus (blocos
  fixos, rayon) com duas partidas: a de hoje (k-means++ do `NumpyRng`, a ordem das somas do numpy, para o
  `metrics.cluster`) e a do scikit-learn guloso (para os mapas). Testes: os do Janus (`tests/kmeans.rs`,
  `cluster.rs:398-440`) trazidos para cá; os de `metrics.*` intactos; 1 e 8 threads dão os mesmos rótulos.
  **Feito**: `crates/ops/src/multivar/` (`linalg.rs`: o `svd` e o `sign_of` vindos do `signatures.rs`, `moments`
  (médias e covariância em uma passada, blocos fixos de 4096 células fundidos em ordem pela fórmula de Chan, iguais
  com 1 ou 8 threads), o Jacobi do Janus e `eigen`; `kmeans.rs`: o k-means do Janus com as partidas `Greedy` e
  `Numpy`, Ward e o corte, e o `kmeans_numpy` do `metrics.cluster`, cujo laço de Lloyd segue o de antes). As tabelas
  de `metrics.*` iguais (os testes de paridade intactos). O `tests/kmeans.rs` do Janus roda um job inteiro (GDAL,
  SOM) e não veio; vieram os testes de unidade do `cluster.rs`.
- **B. `ml.pca`.** Entradas `table` ou `predictors` (pilha) + `cells`; ajustes `components` (um número, ou a parte
  da variância, 0.95 por padrão), `standardise` (correlação, o padrão, ou covariância), `sample`, `seed`. Saídas:
  `scores` (pilha `pc1`, `pc2`..., ou as colunas na tabela), `loadings` (a tabela no formato do `metrics.pca`: uma
  linha por componente, `component` e `explained` e uma coluna por variável, para que o círculo de correlações apareça
  sozinho), `explained` (a tabela da variância para o gráfico de cotovelo) e `model` (`Model::Pca { mean, scale,
  components, explained }` no enum de `ml/mod.rs`, gravado em JSON como os outros). O sinal de cada componente pelo
  `sign_of` (o do `metrics.pca`). Depois: a MNF (Green et al. 1988; a covariância do ruído pela diferença entre
  vizinhos) e o PCA das duas datas para mudança, como opções. Testes: contra o `sklearn.decomposition.PCA` (até o
  sinal) num script de referência como o `tools/make_signature_reference.py`; os scores aplicados a outra imagem
  com a média do ajuste; o gráfico de cotovelo em `charts.rs`.
  **Feito** (sem a MNF nem o PCA de duas datas, que ficam como opções): `multivar/pca.rs` (`fit`, `Pca::scores`, a
  parte da variância pelo menor k com soma acumulada >= a parte) e o operador em `ml/pca.rs`. `scores` (pilha) ou
  `table` (a tabela com `pc1`...: duas portas, não uma, pelo tipo fixo das portas), `loadings` (o formato do
  `metrics.pca`), `explained` (componente, variância, razão, acumulada: todos os componentes) e `model`
  (`Model::Pca(Pca)`: nomes, média, escala, componentes, variâncias). O `ml.predict` aplica o modelo (porta `scores`
  com mapas, `table` com tabela); `ml.importance` e `ml.partial_dependence` o recusam. Acima de 1 milhão de
  registros completos o ajuste usa uma amostra, dita num `ctx.warn` (a única via de notas do nó). O gráfico de
  cotovelo (`Kind::Elbow`) está em `charts.rs`. No `python_script`, `ml.pca` é "sem equivalente" por ora.
  Referência: `tools/make_pca_reference.py` e `tests/data/reference/pca.json`.
- **C. `ml.kmeans`.** Entradas como acima; ajustes `k` (um número, uma lista, ou `auto`: a melhor silhueta numa
  amostra entre 2 e 10), `n_init`, `max_iter`, `tol`, `standardise`, `sample`, `seed`. Saídas: `clusters` (o mapa,
  ou a coluna `cluster` na tabela), `centres` (a tabela: um centro por linha, nas unidades originais, com o tamanho e
  a inércia de cada grupo), `choice` (com uma lista de k: inércia, silhueta e Calinski-Harabasz por k, para o
  gráfico) e `model` (`Model::KMeans { centres, mean, scale }`). Os rótulos em ordem estável (pelo primeiro
  componente dos centros), para que as cores não mudem de uma execução para outra. Depois, como opções: mini-batch
  (Sculley 2010) para amostras enormes, e o ISODATA (dividir e juntar grupos) dos programas de sensoriamento
  remoto. Testes: a inércia contra o `sklearn.cluster.KMeans` com os mesmos centros iniciais; a `nlm.image` (do plano
  dos objetos) separada nas suas classes; a ordem dos rótulos estável.
  **Feito** (sem o mini-batch nem o ISODATA, que ficam como opções): `multivar/clusters.rs` (`Clusters`, o modelo:
  nomes, média, escala, centros nas unidades do ajuste; `assign` por pedaços de 8192 registros, com tamanhos e inércia
  somados em ordem; `order`, a ordem dos rótulos; `silhouette` e `calinski_harabasz`) e o operador em `ml/kmeans.rs`;
  em `multivar/kmeans.rs`, `Settings` (`max_iter` e `tol`), `kmeans_with` e `kmeans_from` (Lloyd a partir de centros
  dados, para comparar com o scikit-learn), e a leitura dos registros de `ml/pca.rs` passou a ser comum aos dois
  operadores. A partida é a gulosa (`Init::Greedy`); o desvio padrão da padronização tem divisor n (como o
  `StandardScaler`). **A ordem dos rótulos**: o primeiro componente principal da covariância dos centros (padronizados),
  com o maior coeficiente positivo (o `sign_of` do `ml.pca`), do menor escore ao maior, empate pela posição. `k` é um
  número, uma lista (cada um >= 2) ou `"auto"` (2 a 10, o padrão); com vários k, cada um é ajustado em até 100 000
  registros sorteados (o escolhido é ajustado de novo em todos) e a silhueta é de uma amostra de até 10 000, a mesma
  para todo k; vence a maior silhueta (o menor k no empate), dito num `ctx.warn`. Saídas: `clusters` (mapa) ou `table`
  (coluna `cluster`), `centres` (cluster, size, inertia -- nas unidades do ajuste, padronizadas -- e os centros nas
  unidades originais), `choice` (k, records, inertia, silhouette, calinski_harabasz; sempre, com uma linha para um k só)
  e `model` (`Model::KMeans(Clusters)`, `"kind": "kmeans"`). O `ml.predict` dá a porta `map` com mapas e a coluna
  `cluster` com tabela. O gráfico da escolha de k (`Kind::Choice`: inércia e silhueta contra k) está em `charts.rs`.
  No `python_script`, `ml.kmeans` é "sem equivalente" (os sorteios do scikit-learn não são os do Terraflux).
  Referência: `tools/make_kmeans_reference.py` e `tests/data/reference/kmeans.json`.
- **D. `ml.som`.** O `som.rs` do Zeit trazido para `multivar/som.rs`, o `parallel::Shared` trocado pelo rayon e o
  `n_jobs` pelas threads do Terraflux, os testes do Zeit junto (inclusive os que conferem os bits contra o MiniSom).
  Ajustes: `rows` e `columns` (o padrão por Vesanto: 5 √n nós, a razão dos lados pelos dois primeiros autovalores do
  PCA), `topology` (retangular; hexagonal é o acréscimo novo), `neighbourhood` (gaussian, bubble, triangle),
  `learning_rate`, `sigma`, `decay`, `training` (batch, o padrão, ou online), `iterations`, `standardise`, `sample`,
  `seed`, e `clusters` (k > 0: os nós do SOM agrupados em k grupos pelo k-means ou pelo Ward da etapa A, a
  abordagem em dois níveis de Vesanto & Alhoniemi 2000, como o Janus faz). Saídas: `nodes` (o mapa do nó de cada
  célula), `groups` (o mapa dos grupos, quando `clusters`), `codebook` (a tabela: uma linha por nó com linha, coluna,
  os pesos nas unidades originais, o número de células e o grupo), `quality` (erro de quantização e topográfico) e
  `model` (`Model::Som { rows, columns, topology, weights, mean, scale, groups }`). A U-matrix calculada do codebook.
  Testes: os do Zeit; hexagonal contra o MiniSom com `topology='hexagonal'`; o mapa dos grupos reproduzível.
  **Feito**: `multivar/som.rs` (o motor do Zeit copiado com a nota da origem; o `parallel` do Zeit trocado pelo rayon do
  chamador: a época em lote passa por três laços paralelos, os vencedores em pedaços de 512 registros, as somas por
  blocos fixos de 8 nós, cada um na ordem das amostras, e a atualização em série; os mesmos bits com 1 ou 8 threads;
  `Grid::minisom` com as coordenadas do MiniSom, hexagonal incluída: as linhas contadas da última, uma sim e uma não
  deslocada meio nó, e o espaço entre linhas (3/2)/raiz de 3; `predict_two`, os dois nós mais próximos) e
  `multivar/codebook.rs` (`Som`: nomes, linhas, colunas, topologia, média, escala, pesos e grupos; `assign` por pedaços
  de 8192 registros; `quality` (erro de quantização e topográfico); `umatrix`; `fit`, `vesanto` e `group_nodes`). O
  operador está em `ml/som.rs`. **Decisões**: o nó 1 é o da linha 1, coluna 1, e os números seguem linha por linha (o
  motor do MiniSom numera por coluna e depois por linha: a conversão fica em `fit`); a grade `rows` x `cols` (e não
  `columns`, que já é a escolha das variáveis de tabela nos três operadores); o tamanho de Vesanto é 5 raiz de n nós,
  no máximo 2500, os lados na razão das raízes dos dois maiores autovalores da correlação (ou da covariância, sem
  padronizar), a linha mais comprida como `rows`, e é dito num `ctx.warn`; a partida é a de Kohonen (o plano dos dois
  primeiros componentes, de -2 a 2 desvios, o lado mais comprido no primeiro; `init` = random sorteia registros),
  `learning_rate` 1 em lote e 0,5 em online, `sigma` a metade do lado maior, `iterations` 50 em lote e 5 passadas em
  online (um passo por registro, a ordem sorteada pela semente; no máximo 50 milhões de passos); `decay` escolhe o par
  (taxa, sigma) do MiniSom; `clusters` agrupa os pesos dos nós (não ponderados) pelo Ward (ou k-means) e numera os
  grupos pelo primeiro componente dos centros, como o `ml.kmeans`. O Janus não agrupa os nós: usa cada nó como grupo
  (até 12), então o dois níveis é só o de Vesanto & Alhoniemi. A U-matrix é a distância média de cada nó aos vizinhos
  (os 8 em volta, ou os de distância 1 nas coordenadas hexagonais), na unidade do ajuste, e vai como coluna `umatrix` do
  `codebook` (sem tabela à parte). O `ml.predict` ganhou a porta `groups`. No `python_script`, `ml.som` é "sem
  equivalente" (a partida e o agrupamento não são do MiniSom). **Achado**: a `distance_map` hexagonal do MiniSom
  desloca as linhas pelo outro lado que as suas coordenadas, e por isso a U-matrix hexagonal é conferida contra as
  coordenadas do MiniSom e a retangular contra a `distance_map`. Referência: `tools/make_som_reference.py` e
  `tests/data/reference/som.json` (cada número como os 64 bits: 13 casos, os 8 do Zeit e 5 hexagonais; os pesos treinados
  iguais aos bits do MiniSom no Windows, onde o `exp` do NumPy é o da biblioteca C, e a 1e-12 nos outros sistemas).
- **E. Os gráficos e o Viewer.** Em `charts.rs`: o cotovelo do PCA, a escolha do k (inércia e silhueta por k) e o
  **SOM**: a grade dos nós com a U-matrix, os planos de componentes (uma grade por variável), o mapa de contagens e os
  grupos; um clique num nó (ou num grupo) acende as suas células no mapa do Viewer, e uma célula no mapa mostra o seu
  nó. No Viewer, os scores do PCA como composição RGB (`rgb.rs`: pc1, pc2, pc3) com um clique.
  **Feito**: o cotovelo e a escolha do k já vinham das etapas B e C; agora o SOM e o composto RGB. O `codebook` ganhou a
  coluna `topology` (1 retangular, 2 hexagonal, nomeada pelos `labels` da tabela) entre `umatrix` e os pesos, porque a
  topologia não se infere da tabela; `Kind::Som` em `charts.rs` (a tabela de codebook abre assim, `Chart::first`): a grade
  de quadrados ou hexágonos (`node_centre`, as linhas deslocadas meio nó como `shifted` do MiniSom, a distância (3/2)/raiz
  de 3 entre as linhas hexagonais; `node_at` acha o nó sob o ponteiro) em quatro vistas, `SomView`: U-matrix, planos de
  componentes (uma grade por variável), contagens e grupos, na rampa viridis (os grupos nas cores das classes). O clique
  num nó escolhe `SomPick::Node` (na vista dos grupos, `Group`; de novo, solta). A ligação com o Viewer está em
  `som_link.rs`, pelo nome do nó `ml.som` no contexto do egui: no Viewer, ao mostrar `nodes` ou `groups`, uma camada
  acende as células do nó ou do grupo escolhido (as outras escurecidas), e uma célula clicada no mapa escolhe o seu
  nó (ou grupo) na vista. A tabela e o mapa são saídas diferentes, e o Viewer mostra uma por vez: a escolha vale na
  outra. O composto dos scores do PCA: `rgb::presets` ganhou "principal components (PC1 PC2 PC3)" (pelos nomes pc1, pc2,
  pc3 onde estiverem) e `Rgb::first` o toma, então a caixa RGB das Camadas, de uma pilha de scores, já liga pc1 pc2 pc3,
  cada banda de 2% a 98%. Feito (2026-11-22): cada clique no mapa conta (`MapView::clicks`, que
  `som_link::clicked` compara em vez da célula), então clicar de novo na mesma célula depois de escolher outro nó na
  vista repete a escolha; a vista tem o botão "Show the map of nodes" (ou "of groups", com a vista por grupos) que põe
  a saída `nodes` (ou `groups`) do `ml.som` no Viewer numa só ação.
- **F. Para quem está começando.** Templates (com a `nlm.image` ou pilhas `nlm.*`, rodando como estão, com `data`
  para o *Use my data*): "Principal components of an image", "Unsupervised classification of an image (k-means)",
  "Types of landscape by a self-organizing map" (uma pilha de variáveis ambientais, os grupos como mapa) e "Change
  between two dates by principal components". O guia do MCP (no `data-preparation` ou num tópico próprio,
  `multivariate`), as dicas de `tips.rs` (padronizar, poucos dados para a grade, k grande para a amostra, componentes
  que somam pouca variância), as frases de `phrases.rs`, as citações do `methods.rs` (Pearson 1901, Hotelling 1933,
  Lloyd 1982, Arthur & Vassilvitskii 2007, Kohonen 1982 e 2001, Vesanto & Alhoniemi 2000), e o `python_script`:
  `sklearn.decomposition.PCA`, `sklearn.cluster.KMeans` e `minisom.MiniSom` (este dando os mesmos bits pelo port do
  Zeit).
  **Feito** (as dicas, as frases e as citações dos métodos já vinham das etapas B a E, o `python_script` também): os
  quatro templates em `templates.rs`, no grupo "Surfaces and uncertainty" (PCA, k-means e SOM) e no "Land change" (a
  mudança), todos rodando como estão e com `data` nos nós do usuário: "Principal components of an image" (`nlm.image`
  -> `ml.pca` com 3 componentes, os scores para o composto RGB, `loadings` e `explained` com o cotovelo),
  "Unsupervised classification of an image (k-means)" (`ml.kmeans` com k `auto` e `sample = 4000`: o ajuste numa
  amostra mantém o template rápido mesmo no build de depuração, e a imagem inteira recebe o seu grupo; o mapa, a
  escolha do k e os centros), "Types of landscape by a self-organizing map" (relevo, chuva, temperatura e umidade de
  nlm.gaussian_field como pilha -> `ml.som` 8 x 8 com `clusters = 5`: o mapa `groups` e o `codebook`, que abre como o
  gráfico do SOM; cada variável é `data` de tipo `values`) e "Change between two dates by principal components".
  **Decisão da mudança**: o componente menor não é o último. Com duas bandas (red e nir) em duas datas são quatro
  variáveis, e medido no template o `pc3` é o da mudança (média de |pc3| 1,12 onde a terra mudou e 0,26 onde não; o
  `pc4` é só ruído, 0,13 contra 0,11), com as cargas do nir das duas datas de sinais opostos (o teste confere os
  dois). O template usa `pc3` e diz no `note` e na descrição que o componente da mudança depende das imagens e das
  bandas, e que as cargas o mostram; a segunda data é a de "Land cover change by objects" (nlm.image da mesma
  classificação com uma mudança sorteada: floresta para urbano e pasto para lavoura). **Feito (2026-11-22)**: o PCA de duas datas, como opções do `ml.pca` (entrada `later`, ajuste `change`):
  `stacked` (Byrne et al. 1980; Fung e LeDrew 1987), o PCA das camadas das duas datas (`<banda>_1`, `<banda>_2`), com a coluna
  `change` em `explained` (||a-b||^2 / (||a-b||^2 + ||a+b||^2) das cargas) e um aviso que nomeia o componente mais parecido com
  mudança entre os de 1% da variância ou mais; `selective` (Chavez e Kwarteng 1989), um PCA por banda, `<banda>_pc1` (o que
  ficou) e `<banda>_pc2` (o que mudou), sem `model` (um ajuste por banda; o `ml.predict` não tem o que aplicar). Testes contra o
  scikit-learn (`tools/make_pca_reference.py`, `tests/data/reference/pca_dates.json`). O template usa `later`. A MNF: **Feito (2026-11-22)**, como `method = "mnf"` do `ml.pca`
  (só com `predictors`; recusa `table` e `later`): a covariância do ruído pela diferença com o vizinho da direita (`noise = "shift"`, a de
  `noise_from_diffs(X, "right")` do Spectral Python: covariância das diferenças com n-1, sobre 2) ou a média dela e da diferença com o
  vizinho de baixo (`shift_both`); o branqueamento pelo ruído e o PCA do branqueado, ordenado pelo autovalor (SNR + 1); os scores
  `mnf1`... com o ruído de variância 1 (o `reduce` do SPy); `denoised` (o `denoise(X, num=k)`); o modelo é um `Pca` com prefixo `mnf`,
  que o `ml.predict` aplica. Igual ao SPy a 1e-9 (`tools/make_mnf_reference.py`, `tests/data/reference/mnf.json`). Com amostra,
  o sinal e o ruído vêm dos sorteados e dos vizinhos deles, dito numa nota. O guia do MCP ganhou o tópico `multivariate` em `guide.rs` (quando usar
  cada um, padronizar, as escolhas automáticas, ler os resultados, a mudança entre datas, os objetos), com um
  ponteiro nos itens 7 a 9 do `data-preparation`. O `python_script`: os três ficam em "sem equivalente", com as razões
  de `python/models.rs` relidas (a ordem dos números, o modelo e o mapa que o script não tem): estão claras e ficam
  como estão. **Achado**: o `memory.rs` estimava mal o que as pilhas e o `nlm.image` seguram (o `nlm.image` sem
  `like` caía na grade 256 x 256, e uma camada tirada de uma pilha contava todas as camadas); o teste que compara a
  estimativa com o que os templates seguram (`the_estimate_stays_near_what_templates_hold_when_they_run`) acusou o
  template da mudança, e a estimativa agora toma a grade das `classes` e conta seis bandas para o `nlm.image`, e uma
  camada para o `stack.extract_layer`. Testes: `the_multivariate_templates_run` (a variância explicada, a pureza dos
  grupos contra a verdade desenhada, os cinco grupos do SOM, a mudança) e o da classificação dos objetos em
  `the_object_templates_run`.
- **G. Com os objetos.** A tabela de atributos dos objetos (plano dos objetos, etapa C) é uma tabela: `ml.kmeans` e
  `ml.som` sobre ela são a classificação não supervisionada de objetos, e `ml.pca` reduz os atributos antes do
  `ml.fit`. Um exemplo no template dos objetos e um teste.
  **Feito**: o `objects.classify` (`objects/classify.rs`) aceita o `Model::KMeans` e o `Model::Som`, sem amostras: lê as
  colunas do modelo na tabela dos objetos pelo nome (a mesma checagem de antes, com uma mensagem que manda nomear os
  atributos em `columns`) e dá a cada objeto o seu grupo; as classes são "cluster 1"... para o k-means, "group 1"... para
  o SOM com `clusters` (o grupo do nó) e "node 1"... para o SOM sem grupos; a "probabilidade" é 1 para a classe e 0 para
  as outras (a margem sai 1; o `min` não filtra nada), um objeto com atributo NaN fica sem classe; sem cores (a paleta
  das classes decide). O que o `ml.kmeans` e o `ml.som` dão sobre a tabela dos objetos precisa de `columns` com os
  atributos: a tabela traz também `id`, `cells` e `area`, que entrariam como variáveis. Um teste em `tests/objects.rs`
  (`classify_by_an_unsupervised_model_gives_the_objects_their_clusters`: k = 5 acerta 99,8% das células contra a
  verdade, o SOM de 4 x 4 com 5 grupos 81%, e sem grupos 16 classes) e o template "Unsupervised classes of objects" (k
  `auto` com os atributos `mean` e `ndvi`; o silhouette escolhe 9 grupos, mais que as cinco classes desenhadas, e o
  teste exige entre 4 e 10 e pureza de 85%). O plano B (`ml.predict` mais uma junção) não foi preciso. Feito (2026-11-22): o
  template "Classify objects by principal components of their attributes" (`ml.pca` sobre 29 atributos, 4 componentes,
  `objects.join` põe pc1..pc4 nos objetos por `id`, depois `objects.samples`, `ml.fit` com `columns` os pcN e
  `objects.classify`; exatidão 90% contra a verdade). Faltava um operador que ponha uma tabela nos atributos dos objetos:
  `objects.join` (por `id`, recusa id sem objeto, repetido ou objeto sem linha; `columns`, `replace`).

**Decisões tomadas** (2026-11-19, as propostas):
- `metrics.pca` e `metrics.cluster` ficam como estão, finos sobre o módulo comum.
- Acima de 1 milhão de células o ajuste usa uma amostra, dito numa nota do nó (não um aviso).

## Áreas de vida, UDs, corredores e limpeza de trajetórias (amt e move2utils, pedido de 2026-11-20)

Hoje o Terraflux e o terraflux-py (`tf.move`) têm a base do move2 e a parte de trajetórias e passos do amt (fases 1
e 3 do plano de `extern/terraflux-py/IDEAS.md`, "Ecologia do movimento"; 27 operadores `move.*`, bit a bit com o R).
Este bloco faz a fase 5 daquele plano (as áreas de vida do amt) e a fase 6 (dBBMM e `corridor`), com uma mudança:
**a referência da fase 6 passa a ser o move2utils** (github.com/move2universe/move2utils, 0.4.6, GPL >= 3; Kranstauber,
Safi, Scharf, o grupo do move2), em vez do `move` v1. O move2utils mantém os mesmos núcleos em C do `move`, sobre o
`sf` e o `terra`, e acrescenta o dBGB (a ponte direcional), o `emd` entre UDs e a escolha da janela. Junta a parte
simples da limpeza de trajetórias do move2utils.

Do move2utils, **entra**:

| Grupo | Funções do R | No Terraflux / `tf.move` |
|---|---|---|
| Variância e UD do dBBMM | `mt_dbbmm_variance`, `mt_dbbmm_ud`, `mt_motion_variance`, `mt_suggest_dbbmm_window`, `mt_mask_segments`, o erro de localização (`location_error.R`: número, coluna, Argos LC, `location_error_na`) | `move.dbbmm_variance`, `move.dbbmm_ud`, `move.suggest_window`, `move.mask_segments` |
| Variância e UD do dBGB | `mt_dbgb_variance`, `mt_dbgb_ud` | `move.dbgb_variance`, `move.dbgb_ud` |
| Volume e comparação de UDs | `ud_volume`, `ud_outer_probability`, `emd` (Sinkhorn) | `move.ud_volume`, `move.ud_outer_probability`, `move.ud_distance` |
| Corredores observados | `mt_corridor` (LaPoint et al. 2013) | `move.corridor` |
| Limpeza simples | `v_phys_estimate` (Hirt et al. 2017), `mt_flag_speed_cap` só com `threshold_type = "hard"` (o `v_max` dado ou vindo da massa e do modo), `mt_filter_gps_quality` | `move.max_speed`, `move.flag_speed`, `move.filter_gps_quality` |
| Desbaste | `mt_thin_time` (programação dinâmica com tolerância: não é o `filter_per_interval`), `mt_thin_distance` (`step` e `interval`) | `move.thin_time`, `move.thin_distance` |

**Fica de fora** (decidido com o usuário em 2026-11-20): a cascata de outliers (`mt_clean_track`, os primitivos
bridge / detour / probabilístico, `mt_flag_outliers_dbgb/_dbbmm`, `mt_peel_speed`, `mt_sequential_outliers`,
`mt_combined_outliers`, `mt_flag_consensus`, `mt_persistence_score`, `mt_diagnose_clean_track`), os limites
automáticos de velocidade (`threshold_type` `auto` / `entropy` / `gap`, o teste do dip, `mt_suggest_speed_cap`) e o
`pool_by`. É ciência nova, descrita num preprint (Safi 2026, bioRxiv 10.64898/2026.07.11.737894) ainda em revisão,
com a API mudando (o padrão do `consensus` mudou na 0.4.x) e com dois modos de falha documentados no próprio README
(marca como outliers os pontos de um estado minoritário, como os 12,7 % de voo de corrida do pombo; deixa passar os
blocos de pontos falsos sem `mass` e `mode`). Rever quando o artigo for publicado e a versão estabilizar. O `hr_akde`
e o `hr_od` do amt continuam na fase do ctmm.

Do amt 0.3.1 (fase 5), **entra**: `hr_mcp`, `hr_kde` com as larguras de banda `hr_kde_ref`, `hr_kde_ref_scaled`,
`hr_kde_pi` (o `dpik` do KernSmooth) e `hr_kde_lscv`, `hr_locoh` (`k`, `a`, `r`; o FNN), `hr_isopleths`, `hr_area`,
`hr_ud` / `hr_cud`, `hr_overlap` (`hr`, `phr`, `vi`, `ba`, `udoi`, `hd`) e o `intensity_use` que faltou da fase 3.

**Plano de implementação** (levantado no código em 2026-11-20). Valem para todos:
- **Como sempre, primeiro o terraflux-py, depois o Terraflux.** Em `extern/terraflux-py`: o C++ em `src/cpp/move/`
  (o `movecore`, com o pybind em `python.cpp`), o Python em `src/terraflux/move/` (um arquivo por grupo:
  `_homerange.py`, `_ud.py`, `_corridor.py`, `_clean.py`), os casos em `tests/move_cases.py`, o R em
  `tools/move_reference.R` (mais `tools/move2utils_fixed.R` para os erros achados), as referências gravadas por
  `tools/make_reference_move.py` (`tests/data/reference/move/amt_hr.npz`, `move2utils.npz`), os testes em
  `tests/test_move_hr.py`, `test_move_ud.py`, `test_move_clean.py`, a página `docs/api/move.md` e o CHANGELOG. No
  Terraflux: as funções do `movecore` no `crates/ls-sys` (`src/lib.rs`, `shim/shim.cc`, `shim.h`), os operadores em
  `crates/ops/src/movement/` (arquivos novos `homerange.rs`, `ud.rs`, `corridor.rs`, `clean.rs`, registrados no
  `mod.rs`), os testes em `crates/ops/tests/movement.rs` contra as referências do terraflux-py
  (`tools/make_move_reference.py`), o mapeamento do `python_script` em `crates/app/src/python/movement.rs`, as
  entradas em `tips.rs` e `phrases.rs`, o `essentials.rs` para os de mais de seis parâmetros, as referências no
  `methods.rs`, os conselhos no `advice.rs`.
- **Paridade**: bit a bit com o R onde a conta é determinística (todas deste bloco menos o Sinkhorn, por tolerância
  de 1e-12 relativa). O dBBMM também é conferido contra o `move` v1 (já na biblioteca R), como o move2utils faz nos
  seus `test-move-parity-*`. Erros das referências: só a forma correta, testada contra a referência corrigida.
- **Núcleos**: o C++ recebe `threads` (o teto central do Terraflux, `threads::for_this_thread`; no terraflux-py,
  `n_workers` e `TERRAFLUX_THREADS`); o `bgb_bbmm_omp.c` do R não é copiado como está: o paralelo é por tiras de
  linhas da grade, cada célula somando os segmentos na ordem da trajetória, para o mesmo resultado com 1 e 8 threads
  (testado). As trajetórias rodam em paralelo entre si, montadas na ordem dos ids.
- **CRS**: como o move2utils, lon/lat é projetado por dentro (o `aeqd_crs` corrigido, de cada trajetória) e o UD
  volta no CRS da entrada quando ela era lon/lat, pelo mesmo reamostrador do `terra` usado lá (`.warp_ud_to_target`:
  conferir na fase 0 qual método e portar; senão, UD sempre no CRS projetado com uma nota).
- **Tipos, nenhum novo**: a variância vai como colunas das trajetórias (`motion_variance`, `in_windows`,
  `interest` e, no dBGB, `para_sd`, `orth_sd`), e o UD é um raster (uma banda por trajetória, com os nomes das
  trajetórias; probabilidade por célula somando 1). As isopletas e as áreas de vida saem como vetor de polígonos
  (`track`, `level`, `area`).
- **Grade do UD** (`utils.R::.ud_grid_args`, `ud_grid_probe.c`): uma entrada opcional `like` (o mapa cuja grade usar,
  como no `data.rs`) ou os ajustes `res`, `dim_size`, `ext_factor`; a grade cresce sozinha quando as pontes não cabem,
  como no R, com um `ctx.warn`.
- Um commit por fase em cada repositório (o do terraflux-py primeiro, depois o do Terraflux com o submódulo
  atualizado), nesta ordem: 0, 1, 1b, 2, 3, 4, 5, 6, 7. Sem push nem release até o usuário pedir; no release, o
  `website/data/ops.json` atualizado (os `move.*` novos).

- **0. Harness e leitura (S).** Instalar o move2utils 0.4.6 (fixar o sha do commit) na biblioteca
  `%LOCALAPPDATA%/R/landschaft-move-lib`. Ele pede R >= 4.5 e a máquina tem o R 4.4.2 com o Rtools44: primeiro
  instalar do fonte com o `Depends` relaxado numa cópia (o código não parece usar nada do 4.5); se falhar, o R 4.5 e o
  Rtools45 na pasta do usuário (sem admin). Mais o `emdist` e o que faltar do amt (FNN já está; KernSmooth vem com
  o R). Dados: os fishers (já em `tests/data/move/fishers.csv.gz`) e o `synthetic_tracks.csv.gz` do move2utils (com
  os outliers conhecidos, para a velocidade e o desbaste); o `Leo-65545` e o `Pettstadt1-14053` só se a licença do
  estudo do Movebank permitir (conferir). Ler os núcleos em C (`bm_variance_c.c`, `bgb_var_window_c.c`, `bgb_bbmm.c`,
  `bgbVar.c`, `ud_grid_probe.c`) e anotar as manhas: tempo em minutos, o BIC dos pontos de quebra, a expansão do erro
  de localização, o passo de tempo da integração (`time_step`), a ordem das somas.
- **1. Limpeza e desbaste (S). Feito (2026-11-20):** terraflux-py b74c9ad, Terraflux a0716f5 (`move.max_speed`, `move.flag_speed` com `mass` e `mode` obrigatórios juntos, `move.filter_gps_quality`, `move.thin_time`, `move.thin_distance`; dois erros do move2utils testados contra `tools/move2utils_fixed.R`). Achado no caminho: os sorteios da gama não eram seguros entre threads (o `current_rng` global e os `static` do `rgamma.c` do R), o que derrubava os testes em paralelo e podia derrubar dois modelos rodando juntos; corrigido com `thread_local` (terraflux-py 51acd76) e um teste de regressão. `v_phys_estimate` (a fórmula de Hirt et al. 2017 com os parâmetros da Tabela S4, o
  intervalo pelo método delta) como `move.max_speed` (massa em kg e modo → m/s, com o intervalo) e como ajuste
  `mass` / `mode` do `move.flag_speed`; `move.flag_speed` (o `hard` do `mt_flag_speed_cap`: a coluna `speed_flag`,
  ou remove, com `v_max` ou `mass` + `mode` e o fator 1,25 do README); `move.filter_gps_quality` (`sat_min`,
  `dop_max`, `hacc_max`, as colunas pelos nomes do Movebank e por ajuste); `move.thin_time` e `move.thin_distance`
  (a programação dinâmica em C++, linear, para trajetórias de 1e5+ pontos; a coluna `thin_selected` ou só os
  escolhidos). Testes contra o R nos fishers e na trajetória sintética.
- **1b. Os valores ao longo do passo (S). Feito (2026-11-21):** terraflux-py 402a483 (`extract_covariates_along`, bit a bit com o amt e o terra 1.9-50: a grade recortada e o queimador `ALL_TOUCHED` do GDAL 3.9.3 portados; `along_summary`), Terraflux (`where = "along"` e `stats` no `move.extract_covariates`, a saída `along`). Ressalva: o texto de 15 dígitos do `paste0` do R sai do `formatReal` em long double; o `%.15g` difere dele em ~1 % dos números aleatórios de 15-16 dígitos (1e-14 relativo), o que só muda as células de uma coordenada exatamente numa aresta; emular o `formatReal` se isso importar. O `extract_covariates_along` do amt que faltou da fase 3: as células que
  cada passo atravessa (a linha do início ao fim, na ordem do amt: conferir no R se é o `terra::extract` sobre a
  linha e em que ordem vêm as células) e os seus valores. O amt devolve a lista de valores por passo; o `tf.move`
  devolve igual (`extract_covariates_along`, bit a bit) e, a mais, os resumos por passo que a iSSF usa
  (`along_summary`: média, mínimo, máximo, a subida e a descida acumuladas, o desnível |fim − início|), como colunas
  `<covariável>_along_<resumo>`. No Terraflux, o ajuste `where = "along"` do `move.extract_covariates` com a lista dos
  resumos. É o que mede se um passo corta o morro ou segue a curva de nível (o bloco seguinte).
- **2. Áreas de vida do amt (M). Feito (2026-11-21):** terraflux-py 59fb529 e bd396a6, Terraflux (`move.home_range` com `method` mcp / kde / locoh, `bandwidth` ref / ref_scaled / pi / lscv / número / [x, y], `like` ou `res` e `factor` para a grade, as saídas `polygons`, `table`, `ud`, `bandwidths`; `move.home_range_overlap` com `polygons` ou `ud`; `move.intensity_use`). Os polígonos, as áreas, as larguras de banda e os UDs batem bit a bit com a biblioteca, o LoCoH e a sobreposição de polígonos também (o GEOS 3.14.1 do GDAL contra o 3.13.1 do shapely); os índices de sobreposição de UDs pedem a tolerância de 1e-12 relativa porque o UD sai normalizado e é normalizado de novo. O `ref_scaled` busca o isopleto do maior nível. O `crates/gdal/src/geos.rs` ganhou polígonos, envoltória convexa, buffer, união e interseção. `hr_kde_lscv` com `range` e `hr_kde_pi` com os argumentos do `dpik` ficam só no `tf.move`. O plano era: `hr_mcp` (o `chull` do R e os níveis pela distância ao centroide), `hr_kde` (as
  quatro larguras de banda; a soma dos núcleos por tiras da grade), `hr_locoh` (os três tipos, as uniões de
  polígonos pelo GEOS que o Terraflux já usa; conferir a ordem das uniões do `sf`), `hr_isopleths`, `hr_area`,
  `hr_ud`, `hr_overlap` (os seis tipos, `conditional`) e `intensity_use`. Operadores: `move.home_range`
  (`method` = `mcp` / `kde` / `locoh`, `levels`; saídas `polygons` e, no `kde`, `ud`), `move.home_range_overlap`
  (tabela por par) e `move.intensity_use`. No `tf.move`: as funções do amt com os mesmos nomes (`hr_mcp`...) e o atalho
  `home_range(method=...)` do plano original.
- **3. dBBMM (L). Feito (2026-11-21):** terraflux-py 87e78a1, Terraflux (`move.dbbmm_variance`, `move.dbbmm_ud`,
  `move.suggest_window`, `move.mask_segments`, `move.ud_volume` e `move.ud_outer_probability`, em `movement/ud.rs`;
  quatro operadores, não um `model`: a margem padrão (11 e 15), as colunas e os conselhos são de cada modelo, e as
  oito coisas do plano ficaram em oito operadores pequenos). A variância vai como colunas das trajetórias, o UD é
  um `Stack` com uma banda por trajetória (ou burst, `trilha__burst`), a soma 1 por banda, na grade do `like` ou na
  de `res` / `dim_size` / `ext_factor`, que cresce sozinha quando as pontes não cabem (só avisa com `verbose`, como o
  R). Bit a bit com a biblioteca em todos os casos (20 de variância e 26 de UD: res, dim_size, a grade ampliada,
  `like`, `time_step`, bursts, erros Argos e e-obs, `location_error_na`, máscaras), a partir das coordenadas
  projetadas. De lon/lat: as coordenadas AEQD do Rust (`geometry::aeqd`, a corrigida, a mesma de `tf.move.aeqd_crs`
  na biblioteca) saem idênticas bit a bit às da biblioteca nos 4716 números dos conjuntos de teste (o PROJ do GDAL
  daqui e o do pyproj coincidem); contra o R a diferença vem do PROJ (9.5.1 do pyproj contra 9.4.1 do R: ~1e-7 na
  variância), então os testes de lon/lat aceitam tolerância. `crs = "input"` volta ao CRS das trajetórias pelo
  `gdalwarp` bilinear (zera os negativos e renormaliza; aqui deu os mesmos bits da biblioteca, o teste aceita
  tolerância). Com as colunas da variância e lon/lat, a projeção do centroide é recalculada a partir das trajetórias
  recebidas (o Rust não tem `attrs` para guardá-la): se o usuário filtrar linhas entre a variância e o UD, o centro
  muda um pouco. Tempos (da biblioteca, com o mesmo C++; no Terraflux não medi): os fishers, variância de todos
  21,9 s numa thread e 3,3 s em 8 (R: 24,1 s); UD do M1 em res 100: 1,14 s e 0,25 s (R: 1,47 s); trajetória de 100 mil
  pontos, variância 64 s e 10 s (R: 62 s), UD numa grade 2000x1669 41,5 s e 19,6 s (R: 45,9 s). Erros do move2utils
  achados no caminho (a biblioteca faz o certo): o `mt_aeqd_crs` do move2 centra a projeção num ponto movido ao
  EPSG:4236 (~1 km), o `location_error = "auto"` e os nomes de coluna não acham o `eobs:horizontal-accuracy-estimate`
  do Movebank, e o laço do núcleo que chega a `xEnd == nX` lê uma célula além da grade (aqui é uma janela que não
  cabe). As isopletas do dBBMM como polígonos existem como `levels` do `move.ud_volume` (o caminho do
  `hr_isopleths`; sem referência na biblioteca, testadas por propriedades); falta medir os 100 mil pontos no
  Terraflux. O plano era: A variância por janela móvel (`bm_variance_c.c`, os pontos de quebra pelo BIC, `window_size`
  ímpar, `margin`), o UD (`bgb_bbmm.c` no modo isotrópico: a ponte browniana integrada no tempo com o erro de
  localização), `mt_motion_variance`, `mt_suggest_dbbmm_window` (a sugestão pelo intervalo típico e `target_hours`),
  `mt_mask_segments` (zerar a variância de segmentos escolhidos, por exemplo as lacunas longas: o UD que respeita as
  lacunas da vinheta `UD_gap_aware_ud`), `ud_volume` (o volume cumulativo, para as isopletas do dBBMM como polígonos
  pelo mesmo caminho do `hr_isopleths`) e `ud_outer_probability`. Por burst como na vinheta `UD_bursted_uds`, pelos
  bursts que o `move.resample` já dá. Teste de desempenho: os fishers e uma trajetória de 100 mil pontos numa grade
  de 2000², 1 e 8 threads, contra o R.
- **4. dBGB (M). Feito (2026-11-21):** terraflux-py 97b9593, Terraflux (`move.dbgb_variance`, `move.dbgb_ud`: as
  colunas `para_sd`, `orth_sd`, `n_estim`, `seg_interest` e `dbgb_location_error`, o UD só do núcleo de cada
  trajetória, janelas de cinco desvios padrão, `time_step` padrão `min(diff(t)) / 20.1`). Bit a bit com a biblioteca
  nos casos de variância e de UD (a célula que difere 1 ulp do R no `gud_m1_like` é da biblioteca contra o R, com o
  `acos` da UCRT; contra a biblioteca o teste do Terraflux dá os mesmos bits, inclusive nesse caso); o dBGB pede
  modelos maiores que o dBBMM (o erro diz o tamanho do `like`) e trajetórias longas. Testes iguais aos da fase 3
  (casos de variância e de UD, bursts, máscaras, 1 e 4 threads, erros). O plano era: `bgb_var_window_c.c` (as variâncias paralela e ortogonal, `.delta_para_orth`) e o UD direcional
  pelo mesmo núcleo do passo 3. Mesmos testes.
- **5. Corredores e distância entre UDs (M). Feito (2026-11-21):** terraflux-py b2ecab8 (`corridor`, `corridor_lines`, `ud_distance`; o buffer de 120 vértices do GEOS 3.12 e o ponto no anel portados bit a bit, Sinkhorn bit a bit no BLAS de referência, o exato do emdist 0.3-3 vendorizado; dois erros do `mt_corridor` testados contra o código corrigido: o raio em metros num CRS em pés e o EPSG:4236), Terraflux (`move.corridor` com as saídas `lines` e os limiares, `move.ud_distance`). Ressalvas: de lon/lat o azimute e a variância circular da biblioteca diferem do R em ~1e-7 (PROJ 9.5.1 do pyproj contra 9.4.1 do R; o resultado do corredor é o mesmo); a velocidade usa o intervalo exato em ns, como o `speed`, e não a conversão de minutos do difftime (1 ulp). O manual diz que `min_segments` conta os vizinhos que qualificam; o código conta todos, como aqui. `mt_corridor`: a velocidade e a variância circular dos segmentos
  vizinhos (o R-tree do `sf` vira o índice espacial que o Terraflux já tem), os limites (`speed_threshold`,
  `circvar_threshold`, `min_segments`) com os padrões do R, a coluna `corridor` por segmento e as linhas dos
  segmentos de corredor. `emd` pelo Sinkhorn (`reg`, `max_iter`, `tol`, `mask_quantile`, o custo euclidiano ou o de
  grande círculo `gc`) como `move.ud_distance` (tabela por par de bandas); o método exato (o `emdist`, código de
  Rubner) só se a licença dele permitir portar, senão fica de fora.
- **6. Do movimento para a paisagem (M). Feito (2026-11-21):** 6a, `move.validate` (commits b5a96d9 e 6814e17; as
  definições são nossas, porque só o resumo de Zeller et al. 2018 foi lido; as portas a jusante não precisaram de
  mudança: as isopletas e os polígonos de `move.home_range` e `move.ud_volume` entram como vetor nas métricas e na
  priorização como saem, e as linhas de `move.corridor` pedem `vector.buffer` antes do `metrics.sample`). 6b: três
  templates novos no grupo *Movement* (`crates/app/src/templates.rs`, teste `the_movement_templates_run`): "Home range
  of a track" (MCP, KDE e dBBMM com contornos de 0,5 e 0,95, as áreas em três gráficos de barras),
  "Observed corridors against current" (dBBMM, `move.corridor`, `circuit.omniscape`, `move.validate` do UD e das
  linhas; a resistência do `nlm.gaussian_field` marcada como dado do "Use my data") e "Cleaning a track" (GPS,
  `move.flag_speed` com o limite de `move.max_speed`, `move.thin_time`; os fixes ruins entram por `table.calc`);
  o tópico `movement` do guia do MCP (`crates/app/src/guide.rs`, também na lista das instruções, onde faltava o
  `multivariate`); e os conselhos do `check_model` (`movement_advice` em `crates/app/src/advice.rs`: dBBMM e dBGB sem
  `location_error`, janela par ou menor que 2 margens + 1, janela sem folga ou curta, grade com mais de 4e6 células,
  amostragem que nada no modelo olha, `move.flag_speed` sem limite ou com dois e sem o filtro de qualidade do GPS).
  O plano era: `move.validate`: o UD (ou os pontos, ou os segmentos de corredor) contra
  uma superfície de conectividade (a corrente do Circuitscape / Omniscape, a resistência, um corredor de menor
  custo): a correlação de postos de Spearman entre o uso e a superfície, a fração do UD dentro dos corredores e a
  razão de uso entre dentro e fora (Zeller et al. 2018), numa tabela e numa nota. As áreas de vida e as isopletas
  entram como zonas nas métricas de paisagem e como feições na priorização (conferir que as portas aceitam o vetor
  como sai; ajustar o que não aceitar). Templates novos (com dados do `nlm.*` e uma trajetória simulada pelo
  `move.simulate_brownian`, prontos para o "Use my data"): área de vida de uma trajetória (MCP + KDE + dBBMM, áreas
  num gráfico), corredores observados contra a corrente (dBBMM, `move.corridor`, Omniscape, `move.validate`) e
  limpeza de trajetória (GPS, velocidade, desbaste). Um tópico novo `movement` no guia do MCP (não existe: o método
  do Movebank à validação, com os operadores e os templates) e os conselhos do `check_model` (o `location_error`
  obrigatório no dBBMM, a janela ímpar e maior que a margem, o intervalo de amostragem irregular, a grade grande
  demais, a velocidade máxima sem massa nem modo).
- **7. Documentação (M). Feito (2026-11-21):** as partes 6 (`home-ranges`), 7 (`corridors-and-validation`) e 8 (`cleaning-and-thinning`) da série, com as figuras e os dados dos scripts (cada script roda a biblioteca e os mesmos modelos pelo Terraflux e compara); a página da API do Python, sete linhas da acurácia, os tempos das pontes contra o R e o `website/tools/make_accuracy_view.py` (f072bb7); os CHANGELOGs postos em ordem. Achados dos tutoriais: o UD dos fishers não acompanha a corrente do Omniscape (rho 0,05) e os segmentos de corredor sim (3,5 vezes); nenhum sinal de que sigam as curvas de nível (subida de 4,3 m contra 3,8 m nos passos aleatórios), o que o bloco do relevo retoma. Plano original: Três partes novas da série `website/content/tutorials/animal-movement/` (áreas de vida e
  UDs; corredores observados e a validação da conectividade; limpeza e desbaste), com as figuras pelo `movefig.py`;
  a página da API do Python (`website/content/python/api/move.md`), as linhas do `accuracy_tools.csv` e do
  benchmark de acurácia (contra o amt, o move2utils e o move), os tempos contra o R, as referências (Horne et al.
  2007, Kranstauber et al. 2012 e 2014, LaPoint et al. 2013, Hirt et al. 2017, Rubner et al. 2000, Cuturi 2013,
  Zeller et al. 2018, Signer et al. 2019, Kranstauber et al. 2026 bioRxiv 10.64898/2026.07.07.736908) e os
  CHANGELOGs dos dois repositórios.

**Decisões tomadas** (2026-11-20, as propostas):
1. A variância como colunas das trajetórias, sem tipo novo; `move.dbbmm_ud` aceita trajetórias com ou sem essas
   colunas (sem elas, calcula com os mesmos ajustes, como o `mt_dbbmm_ud.move2`).
2. Várias trajetórias: uma banda por trajetória; somar ou ponderar as bandas fica para o `raster.calc`.
3. O UD volta no CRS da entrada como no R (ou sempre no projetado, se o reamostrador do `terra` não der para portar
   bit a bit). **Resolvido na leitura dos núcleos (2026-11-20):** a volta a lon/lat é o `terra::project` (GDAL e
   PROJ escolhem a grade e reamostram, depois zera os negativos e renormaliza): não é portável bit a bit. O UD sai no
   CRS projetado (o AEQD corrigido da trajetória), com uma nota; a reprojeção para lon/lat, quando pedida
   (`crs = "input"`), pelo `raster.reproject` bilinear com a mesma limpeza e renormalização, testada por tolerância.
4. O harness com o move2utils no R 4.4.2 de `Depends` relaxado antes de instalar o R 4.5. **Feito:** instalado
   (commit b3304d6 do move2utils, `%LOCALAPPDATA%/R/move2utils-install.R`); o exemplo do README roda.
5. O `emd` exato só se a licença do `emdist` permitir. **Resolvido:** o `emdist` 0.3-3 é MIT; o exato entra.

**Notas da leitura dos núcleos** (2026-11-20; as manhas que valem para as fases 3 a 5):
- A referência bit a bit é o próprio move2utils (ele não é bit a bit com o `move`: os seus testes de paridade usam
  1e-6 a 1e-2); o `move` só confere por tolerância. As referências do R rodam com `OMP_NUM_THREADS=1` (a soma por
  ponto da variância usa `omp atomic`: com mais threads nem o R se repete); no C++, a soma por janela em ordem crescente.
- Tempo em minutos desde 1970; o otimizador é um Brent próprio do C (`[0, 1e15]`, `tol = 1e-8`, o `golden =
  0.3819660` truncado, 500 iterações); o BIC `2f + log(ws)` contra `2(f_b + f_a) + 2 log(ws)`, só quebra se menor; o
  `break_list` não é deduplicado (apesar do comentário). O R chama o `dbbmm2_omp`, que paraleliza as células de cada
  passo de tempo: tiras de linhas com cada thread percorrendo todos os passos na ordem dá o mesmo resultado bit a bit.
  `time_step` = `min(time_lag)/15` no dBBMM e `min(diff(t))/20.1` no dBGB; a normalização pelo `sum` em long double.
- O dBGB precisa de um `acos` como o do mingw (x87), que o `rmath.hpp` não tem; o `cumsum` em long double também falta.
- O `mt_corridor` usa o `st_buffer` do sf (30 segmentos por quadrante) com raio de meio segmento e o `st_intersects`
  com os pontos médios de todas as trajetórias juntas: para bit a bit, o mesmo buffer do GEOS, não um círculo exato.
  Os limites padrão são o `quantile` tipo 7 (0,75 da velocidade, 0,25 da variância circular).
- O Sinkhorn passa pelo `dgemv` do BLAS de referência: tolerância de 1e-12, como previsto.

### O bloco seguinte: o movimento no relevo (pedido de 2026-11-20)

A pergunta do usuário: um animal como a cabra-da-montanha anda ao longo dos morros em vez de subir e descer em linha
reta; os modelos consideram a topografia? Os deste bloco não: o dBBMM, o dBGB, o KDE e o MCP descrevem onde o animal
esteve, e a ponte entre duas posições é browniana no plano (com posições espaçadas, o UD põe probabilidade por cima do
vale). A topografia entra pela **seleção de passos** (iSSF; Avgar et al. 2016, Signer et al. 2019) e pelo **custo
energético** (enerscape; Berti et al. 2022, Pontzer 2016). Depois deste bloco, na ordem:

1. **iSSF** (a fase 4 do plano do `tf.move`, em `extern/terraflux-py/IDEAS.md`): `fit_issf` / `fit_clogit` (o
   `clogit` do survival portado, Newton-Raphson, bit a bit), `log_rss`, `issf` (uma chamada), `simulate_path` (o
   núcleo de redistribuição), `resistance` (a seleção relativa virando resistência, Zeller et al. 2012, Keeley et al.
   2016). As covariáveis de relevo que fazem a diferença, todas já possíveis com o `surface.metrics` (declividade,
   orientação, TPI, TRI, VRM) e o 1b deste bloco: no fim do passo (a declividade, a rugosidade, a distância ao terreno
   de fuga, por exemplo vertentes acima de ~40° para cabras), ao longo do passo (o desnível e a subida acumulada: um
   coeficiente negativo quer dizer que o animal segue as curvas de nível) e as interações com o movimento
   (`log_sl × declividade`, `cos_ta × rugosidade`: passos mais curtos e mais voltas no íngreme). As trajetórias
   simuladas pelo modelo contornam os morros como o animal e viram um mapa de conectividade (Signer et al. 2024).
   Operadores `move.fit_issf`, `move.log_rss`, `move.simulate_path`, `move.resistance`.
2. **enerscape** como `cost.energy_landscape` (o custo de transporte pela declividade e a massa do animal, Pontzer
   2016; bit a bit com o enerscape 1.2.0 no R): como covariável da iSSF e como resistência do caminho de menor custo,
   do Circuitscape e do Omniscape, cujo ótimo segue a curva de nível.
3. **Distância sobre o relevo** (pequeno): o comprimento do passo na superfície (com o DEM ao longo da linha), ao
   lado do planar, para as velocidades em terreno íngreme.
4. **Um template "movimento no relevo"**: DEM → declividade, TRI e custo energético → passos e passos aleatórios →
   covariáveis no fim e ao longo → iSSF → trajetórias simuladas e resistência → corrente; com uma trajetória simulada e
   um DEM do `nlm.*` para rodar como está, pronto para o "Use my data". Mais o tópico `movement` do guia do MCP
   estendido com o relevo e uma parte nova da série de tutoriais de movimento.

**Feito (2026-11-23), tudo.** Na biblioteca (terraflux-py, commits locais): `cost.energy_landscape` bit a bit com o
enerscape 1.2.0 (3224217), `move.surface_distance` (51783b1), `fit_issf` e `log_rss` bit a bit com o amt 0.3.1 e o
survival 3.7-0 (30b0e4d), `redistribution_kernel`, `simulate_path`, `resistance`, `path_density` e `issf` (7a38a01),
e a correção da palavra de controle do RSP que derrubava os testes lado a lado (7acacbf). No Terraflux: os operadores
`cost.energy_landscape` e `move.surface_distance` (38753b15), `move.fit_issf` e `move.log_rss` com o tipo `StepModel`
(c8b8e462), `move.issf_model`, `move.simulate_path`, `move.resistance` e `move.path_density` (96a69b1e), o template
"Movement on the relief", o guia `movement` e quatro conselhos (56ac4ad4), o Python do template rodando inteiro
(1490958c), e a parte 9 da série, "Movement on the relief", com os íbex do Bargy (Marchand et al. 2025) sobre o DEM
Copernicus, mais a parte 5 ajustando com `move.fit_issf` (d583d840). A resposta à pergunta, nos dados: os íbex
escolhem passos que terminam na altura em que começaram (coeficiente da mudança líquida de altura −0,0086, IC −0,0100
a −0,0072, negativo nos quatro), mas não evitam declividade, energia nem rugosidade. No release: refazer o
`website/data/ops.json` (já refeito no d583d840 a partir da árvore).

## O Trace comparado: por que esta célula e não aquela (pedido de 2026-11-21)

O Trace (`crates/app/src/trace.rs`, `trace_view.rs`) responde **como** o valor de uma célula foi calculado: a cadeia
para trás, até 60 cartões (`MOST`), em ordem de busca em profundidade. A pergunta de quem modela, porém, costuma ser
**contrastiva**: "por que esta célula virou cidade e a do lado não?", "por que esta célula muda entre os cenários?".
Hoje a pessoa faria dois traces e compararia de cabeça. Este bloco faz o app compará-los: alinha os dois traces nó a
nó e diz **onde a diferença nasce**. É determinístico, sem IA (como o resto do Trace: o rastreio é do app, a IA só
explica o que ele deu); o "Ask the assistant why" continua para traduzir isso em palavras do domínio.

Duas coisas achadas no código (2026-11-21) mudam o desenho:
- **O `control.switch` escolhe o ramo por execução, não por célula**: o `case` é um valor só (texto, número ou
  booleano; `Engine::switch`, `crates/exec/src/lib.rs` ~2209), ou um por passo quando o switch está num laço. Duas
  células da mesma execução passam sempre pelo mesmo ramo. Os ramos só divergem **entre execuções** (dois
  cenários, outro `case`), e é aí que o switch entra na comparação (fase 5).
- **O trace não entra num `control.switch` hoje**: o `resolve` (trace.rs ~351) atravessa os grupos até o nó que dá
  a saída, mas para no switch, e o `walk` o trata como um operador qualquer. Falta seguir o ramo que rodou (fase 1).

O que já existe e serve: o `Results` do motor tem as mesmas chaves que o `trace::key` e o `Session::value_of` usam
(o nó de topo, e `inner.port` num grupo, `branch.node.port` num switch, mais a saída de texto `branch` com o ramo
que rodou: `crates/exec/src/lib.rs` ~2272-2277); `Model::with_scenario` (`crates/graph/src/lib.rs` ~910) devolve o
modelo com um cenário aplicado, pronto para o `Engine::run`; dentro de um laço, o `Observer::body` recebe o switch
como um nó do corpo, com as saídas `branch.inner.port`, e o `Keeper` (trace.rs ~981) já as guarda como
`sw.branch.inner.port`, o que casa com o `kept_at` em modo laço (só os números: o texto `branch` fica de fora).

**Plano de implementação.** Um commit por fase, nesta ordem; testes, clippy e fmt verdes em cada uma.

1. **O trace segue o ramo que rodou num `control.switch`.**
   - No `walk`: para um switch, o ramo que rodou vem da saída `branch` da execução (`values(sw, "branch")`, ou
     `kept_at` dentro de grupos); o `case` lido como o motor o lê (o texto, o número por `number_text`, `case_X`,
     `otherwise`). Um `How::Branch { case, branch }` novo ("o case era `seco`: rodou o ramo `dry`"), e o trace segue
     para o nó do corpo do ramo que dá a porta pedida (`body.outputs[port]` do grupo do ramo), no caminho
     `[.., sw, ramo]`; os valores pelo `key`, que já dá `(sw, "ramo.nó.porta")`. Os ramos que não rodaram não
     entram.
   - Num laço: o `Keeper` guarda também o ramo de cada passo (hoje só números entram; um mapa à parte, ramo por
     passo, ao lado do `Kept`), e o cartão diz "ramo `dry` nos passos 1 a 4, `wet` a partir do 5"; cada nó do ramo
     traçado com os passos em que o ramo rodou (NaN nos outros, como o `Keeper` já faz com os passos perdidos).
   - O `trace_view::how_ui` e o `Trace::text`/`sentence` dizem o `How::Branch`; o clique no nome de um nó do ramo
     mostra o mapa dele no viewer (o `builder::shown_at` já conhece `branch.node.port`).
   - Testes em trace.rs: um switch num `$case` (o trace entra no ramo certo, nos dois valores do case, e não no
     outro); um switch dentro de um `control.repeat` com o case vindo do passo (o ramo por passo); um switch num
     grupo (`g.sw.branch.inner.port`).
   - **Feito (2026-11-24).** `How::Branch { case, branch, steps }` e `branches_text` ("otherwise at step 1, case_2
     at step 2, otherwise at steps 3 to 5"); o `Kept` virou uma struct (`values`, e `branches` com o ramo de cada
     passo, o texto `branch` que o `Keeper` agora guarda); as séries de um laço igualadas ao fim (NaN nos passos em
     que um ramo não rodou). Duas coisas a mais que o ramo pedia: o trace segue os `@nome` de grupos e switches até o
     que eles recebem (`followed`: o `@f` do ramo, o `@farm` do switch, o `farmland.map` de fora; antes parava na
     borda do grupo), e uma entrada `$nome` de valor mostra o valor (`$weight` = 0.8). A chave do que um laço guarda
     passa por grupos e switches do corpo (`kept_key`: `sw.case_2.up.map`). Testes:
     `a_switch_is_traced_into_the_branch_that_ran` (o template do switch, os dois cenários),
     `a_switch_in_a_loop_is_traced_into_the_branch_of_each_step`, `a_switch_in_a_group_is_traced_through`.

2. **A comparação de dois traces, pura e testável (`trace::compare`).**
   - `compare(a: &Trace, b: &Trace) -> Comparison`: os passos alinhados por `(path, node, port)`; cada par
     **igual**, **diferente**, **só em A** ou **só em B** (os ramos de um switch que divergiu, na fase 5). A
     igualdade de `At` é exata, com NaN igual a NaN (o mesmo cálculo dá os mesmos bits); `Layers` camada a camada,
     `Steps` passo a passo, `Other` pelo texto.
   - **A raiz da diferença**: um par diferente cujas entradas traçadas são todas iguais, ou que não lê nenhum nó.
     O porquê dela, pelo `How` do nó:
     - `Data` (lido ou desenhado): "os dados diferem ali" (as duas células têm valores diferentes no mapa lido);
     - `Window`: "as mesmas entradas na célula, mas as células em volta diferem", com as duas janelas lado a lado e as
       células que mudam marcadas;
     - `Whole` e `Allocation`: "decidido pelo mapa inteiro", com as linhas de cada uma (`clue_why`, `patcher_why`: a
       aptidão, a posição entre as células, a demanda do passo), o que costuma ser o porquê de verdade numa alocação;
     - `Patch`: "manchas diferentes" (a classe, o tamanho e o vizinho de cada uma);
     - `Local` com entradas iguais e valor diferente: "depende de onde a célula está" (um operador de posição ou de
       sorteio; a frase do operador pelo `first_sentence`);
     - uma entrada sem valor guardado, ou o trace cortado em `MOST`: "não se sabe daqui" (com o nó onde parou).
   - As frases saem de moldes, como a `sentence` de hoje: "A (linha 40, coluna 12) = 2 e B (linha 40, coluna 13) = 1.
     Leem o mesmo `landuse_2000` (1); diferem a partir de `suit` (0.71 contra 0.38)."; e, por cartão, os dois
     valores, e as duas expressões de um `raster.calc` com os valores no lugar (`substituted` de cada lado).
   - `Comparison::text()` e `sentence()` para o Copy, o assistente e o MCP.
   - Testes: duas células do CLUE-S (o template, como `a_cell_is_traced_back_through_the_graph`) com a raiz na
     aptidão e na alocação; um modelo pequeno feito à mão para cada tipo de raiz (dado, janela, mancha, local, entrada
     sem valor); duas células iguais ("nada difere"); a mesma célula duas vezes.

3. **O painel: "Compare with another cell".**
   - No Trace, um botão "Compare with another cell" arma a escolha: o próximo clique numa célula do viewer é B
     (um campo novo no `MapView`, ao lado do `picked` de map_view.rs ~629, sem mexer no clique comum). No menu do
     clique direito (map_view.rs ~2676-2702), "Compare with the traced cell" quando já há um trace. O `Ask` ganha a
     célula B; o `State` guarda os dois traces e os refaz juntos quando o modelo ou a execução mudam (o `refresh`).
   - A vista: a frase no topo; **"Where the difference begins"**: os cartões das raízes primeiro, A e B lado a lado;
     depois a cadeia, cada cartão com as duas colunas, os diferentes marcados e os iguais recolhidos ("12 steps the
     same", com "show all"); trocar A e B; Copy; "Ask the assistant why" manda a comparação com a pergunta.
   - No viewer, as duas células contornadas (duas cores, A e B, as do tema), e no clique num cartão o mapa do nó
     com as duas marcadas (o `preview_view.picked` vira um par).
   - Teste de UI no `crates/app/tests` (o programa `app`, com o `point_settings`): traçar uma célula de um template,
     comparar com a vizinha, ver a raiz no painel.

4. **Dentro dos laços, para as duas células.**
   - `keep_loops_at(m, base, &[(x, y)]) -> Vec<Kept>`: o `Keeper` guarda vários lugares na mesma execução (uma só
     rodada a mais para A e B; o `keep_loops` de hoje vira o caso de um lugar). "Trace inside the loops" na
     comparação usa os dois.
   - A comparação de séries: o primeiro passo em que A e B diferem ("a diferença começa no passo 7"); a regra da raiz
     aplicada ao corpo **naquele passo** (os valores do passo s de cada nó do corpo). Como até o passo s-1 tudo era
     igual, o que veio do passo anterior (o `@landscape` carregado) é igual no passo s, e a raiz fica dentro do corpo
     daquele passo. Uma alocação diz a demanda e a posição de cada célula naquele passo.
   - Testes: duas células do CLUE-S que se separam num passo do meio; duas que nunca se separam.

5. **Entre cenários: a mesma célula em duas execuções, onde o `control.switch` diverge.**
   - "Compare with a scenario..." no Trace: a lista de `s.source.scenarios` (e "the model as it is"). O lado B roda o
     modelo com `s.source.with_scenario(nome)` numa thread (como o `keep_loops`: o submodelo expandido, as entradas
     por `load_inputs`; com o Keeper junto quando o trace entra nos laços), com spinner e cancelar; os valores de B
     pelo `Results.values` (um `trace::values_of(&Results)` novo no lugar das closures escritas à mão em tools.rs
     ~603 e nos testes de trace.rs). O lado A é a execução que o app já tem. Uma célula diferente em B também pode
     (o lado é "uma execução e uma célula"), mas o padrão é a mesma célula.
   - As raízes novas: **o ramo** (o par do switch com ramos diferentes: "no cenário `seco` o case era `dry` e rodou o
     ramo `dry`; no `umido`, o ramo `wet`", e os nós de cada ramo como "só em A" / "só em B"); **os ajustes** (um nó
     com as mesmas entradas e um ajuste diferente: o `Step` guarda os ajustes que leem `$nome`, resolvidos, para a
     comparação não precisar dos modelos); **uma entrada do modelo** (`$chuva` vale 800 em A e 1200 em B).
   - Testes: um modelo com um switch num `$case` e dois cenários (a raiz é o ramo); um ajuste mudado por um cenário
     (a raiz é o ajuste); um cenário que não muda a célula ("nada difere aqui").

6. **O MCP e o assistente.**
   - `trace_cell` (tools.rs ~2141) ganha `compare_with` (`{row, col}` ou `{x, y}`) e `scenario` (o lado B naquele
     cenário; `scenarios: [a, b]` para dois cenários quaisquer), com `inside_loops` valendo para os dois lados. No
     `terraflux mcp` (o `trace_cell_run`, tools.rs ~597, que roda o modelo até o nó): duas células numa execução, e
     uma execução por cenário. A resposta é a `Comparison::sentence()` mais o `text()`.
   - O `last_trace` guarda também a última comparação feita no app (o `save_last`); o pedido copiado para um
     assistente pelo MCP e o prompt `explain_cell` passam a dizer "compare".
   - O guia do MCP (o tópico `results`) explica quando comparar em vez de traçar uma célula só.
   - Teste em `crates/cli/src/mcp.rs`, ao lado do `a_cell_is_traced`: duas células do CLUE-S, e um modelo com dois
     cenários.

7. **Documentação.** A página do Trace no site (a documentação da aplicação) com a comparação, as raízes e um exemplo
   do CLUE-S (a figura feita pelo binário de release); o CHANGELOG (Unreleased); as marcas de **Feito** aqui.

Fica de fora, para depois: o caminho decisivo de um cartão só (qual condição de um `raster.calc` decidiu e a margem
até virar: "ficaria `t0` se `suit` estivesse abaixo de 0.45"). Ele melhora as frases da fase 2 (a raiz num
`raster.calc` passaria a dizer qual condição virou), mas é um bloco à parte: precisa avaliar a expressão pela
árvore do parser da calculadora, e uma regra de "quem decidiu" por família de operadores (`min`/`max`, `argmax`,
classificadores).

## Análise espacial de vetores como no PostGIS: predicados, vizinho mais próximo, sobreposição e geometria (pedido de 2026-11-23)

O usuário: "o terraflux possui opções de fazer análise espacial com vetores com within, union, etc como no
Postgis/GDAL? Existe isso na calculadora de vetores?", depois "seria bom ter isso tb em python... funções como essa
diretamente no python assim como existem no GDAL" e "de uma forma que fosse fácil e direta ao ponto".

**O que já existe** (só no aplicativo, Rust sobre o GEOS pelo GDAL; a exportação para Python usa funções privadas
de `python_flow.py` sobre o geopandas): `vector.overlay` (intersection, union, difference, só polígonos),
`vector.spatial_join` (só o predicado *intersects*, a primeira feição ou o resumo), `vector.clip`, `vector.buffer`,
`vector.dissolve`, `vector.geometry` (área, perímetro, comprimento, centroide), `vector.reproject`. **O que falta:**
os outros predicados (`within`, `contains`, `touches`, `crosses`, `overlaps`, `covers`, `covered_by`, `equals`,
`disjoint`, `dwithin`), a seleção por localização, a junção pelo mais próximo, a diferença simétrica e a identidade,
a sobreposição de linhas e pontos com polígonos, as operações de geometria (centroide como ponto, envoltória,
simplificação, retângulo envolvente, `make_valid`, partes separadas, fronteira), um índice espacial (hoje cada par é
testado pela caixa, O(n·m)), e na calculadora de vetores qualquer referência a outra camada: ela só vê os atributos e
as medidas da própria geometria.

**A forma, fácil e direta:** uma função por operação, com os nomes do PostGIS/GDAL, num módulo `tf.vector` da
biblioteca, e um nó com o mesmo nome no aplicativo. Cada função aceita um caminho ou um GeoDataFrame (`b` também uma
geometria do shapely), reprojeta `b` para o CRS de `a` e devolve um GeoDataFrame no CRS de `a`, com os atributos de
`a` (o índice refeito); `out=` grava o resultado (`tf.write_vector`).

```python
import terraflux as tf
v = tf.vector
dentro = v.within("pontos.gpkg", "parques.gpkg")      # as feições de a dentro de alguma de b
perto  = v.dwithin(pontos, rios, 500)                  # a até 500 m de b
fora   = v.select(pontos, parques, "within", invert=True)
v.relate(pontos, parques, "within")                    # um bool por feição de a (o que a calculadora usa)
v.count(parques, pontos)                               # quantas feições de b cada uma de a toca
v.distance(pontos, rios)                               # a distância de cada feição de a à mais próxima de b
v.join(pontos, municipios, predicate="within", how="first" | "all" | "summary", fields=["pop"])
v.nearest(pontos, estradas, max_distance=None)         # os atributos da mais próxima e a coluna distance
v.intersection(a, b); v.union(a, b); v.union(a)        # union de uma camada só: tudo numa feição (ST_Union)
v.difference(a, b); v.symmetric_difference(a, b); v.identity(a, b); v.overlay(a, b, how=...)
v.clip(a, mascara); v.dissolve(a, by="uso"); v.buffer(a, 100, dissolve=True)
v.centroid(a, inside=False); v.hull(a, kind="convex" | "concave", ratio=0.3, by=None)
v.simplify(a, 10, preserve_topology=True); v.envelope(a); v.make_valid(a); v.explode(a); v.boundary(a)
v.measures(a, geodesic=False)                          # area, perimeter, length, x, y
```

Os predicados seguem o sentido do PostGIS e do `sjoin` do geopandas: `within(a, b)` é `ST_Within(a, b)`, a feição de
`a` dentro de alguma de `b`. A biblioteca usa o shapely 2 (o `STRtree` com o predicado, o mesmo caminho do `sjoin`);
o aplicativo chama a API C do GEOS diretamente (`crates/gdal/src/geos.rs` já carrega o `geos_c` do GDAL: um
`GEOSSTRtree` e os predicados preparados `GEOSPrepared*_r`), o mesmo motor. As referências são da biblioteca
(`tools/make_vector_ops_reference.py` passa a chamá-la): as mesmas feições, na mesma ordem, e as áreas e distâncias a
1e-9. O "Python script" do aplicativo passa a escrever `tf.vector.*` no lugar das funções privadas de `python_flow.py`
(que saem quando o nó correspondente passa).

**Sem push nem release** enquanto não for pedido; um commit por fase em cada repositório (a biblioteca no worktree
`terraflux-py-vector`, o aplicativo no worktree `terraflux-vector`, porque outra janela trabalha na árvore principal);
testes, clippy e fmt verdes; até 12 núcleos neste bloco (pedido do usuário), um build pesado de cada vez, até 5
sub-agentes.

### Na biblioteca (`tf.vector`, `src/terraflux/vector.py`)

- **L1. Predicados e junções.** `within`, `contains`, `intersects`, `touches`, `crosses`, `overlaps`, `covers`,
  `covered_by`, `equals`, `disjoint`, `dwithin(a, b, distance)`, `select(a, b, predicate, distance=None,
  invert=False)`, `relate`, `count`, `distance`, `join` (predicate, how first/all/summary, fields, suffix) e
  `nearest` (max_distance, a coluna `distance`, um empate fica com a feição de `b` de menor índice), com os testes
  contra o geopandas (`sjoin`, `sjoin_nearest`) e casos à mão (um ponto na borda: `within` falso, `covered_by`
  verdadeiro, `intersects` verdadeiro).
- **L2. Sobreposição.** `intersection`, `union` (de uma ou duas camadas), `difference`, `symmetric_difference`,
  `identity`, `overlay`, `clip`, `dissolve`, `buffer`; a primeira camada pode ser de linhas ou pontos em
  intersection, difference e identity (como o `keep_geom_type` do geopandas). Testes contra o `overlay` do geopandas.
- **L3. Geometria.** `centroid`, `hull` (o côncavo pelo `concave_hull` do GEOS 3.11+), `simplify`, `envelope`,
  `make_valid`, `explode`, `boundary`, `measures`. A documentação de cada função (docstring com o equivalente no
  PostGIS e no QGIS) e o CHANGELOG da biblioteca.

### No aplicativo

- **A1. Predicados, seleção e vizinho** (depois de L1). Em `terraflux-gdal`: o `STRtree` e os predicados preparados
  pela API C do GEOS. Nós: `vector.select_by_location` (predicate, distance, invert), `vector.spatial_join` com
  `predicate`, `distance` e `mode = all` (uma feição por par, como o `sjoin`), `vector.nearest`. O índice também no
  `vector.overlay` e no `vector.spatial_join` atuais. Python (`tf.vector.*`), tips, phrases, rótulos, methods e
  advice; o teste contra a referência da biblioteca.
- **A2. Sobreposição** (depois de L2). `vector.overlay` com `symmetric_difference` e `identity`, e linhas ou pontos
  na primeira camada.
- **A3. Geometria** (depois de L3). `vector.centroid`, `vector.hull`, `vector.simplify`, `vector.envelope`,
  `vector.make_valid`, `vector.explode`, `vector.boundary`.
- **A4. A calculadora de vetores com outras camadas** (depois de A1). Um vetor ligado a `vector.calc` ou
  `vector.filter` vira um nome do script, com as funções espaciais: `within(parques)`, `contains(...)`,
  `intersects(...)`, `touches`, `crosses`, `overlaps`, `covers`, `covered_by`, `equals`, `disjoint`,
  `dwithin(rios, 500)`, `count(pontos)` e `distance(estradas)`, um valor por feição:
  `within(parques) and geometry.area > 1e4`. Na exportação, `tf.vector.relate`, `count` e `distance`. O guia `calc`
  do MCP e a mensagem de erro que lista os nomes.
- **A5. Documentação.** Uma página do site "Vector analysis" com a tabela de equivalência (PostGIS, GDAL/OGR, QGIS,
  geopandas, `tf.vector`, o nó), a referência Python de `tf.vector`, o guia `data-preparation` do MCP, o template das
  ferramentas de vetor com um predicado e o vizinho mais próximo, o CHANGELOG, o README e as marcas de **Feito**
  aqui. O `ops.json` é refeito na release.

### Mais funções do PostGIS (pedido de 2026-11-23: "pode implementar de 1 até 5 tb")

As que interessam ao Terraflux (ocorrências, rios, estradas, polígonos vindos de raster), na mesma forma: a função em
`tf.vector` e o nó com o mesmo nome. Entram depois de A4 e antes da documentação (A5 passa a ser a última fase e
cobre também estas).

- **L4 / A6. Voronoi, Delaunay e agrupamento.**
  - `voronoi(a, clip=None, tolerance=0)`: um polígono por ponto, com os atributos do ponto, na ordem de `a`
    (`ST_VoronoiPolygons`; o GEOS 3.12 dá as células na ordem dos pontos, antes disso pela posição do ponto em cada
    célula), recortado a `clip` (polígonos) ou à caixa dos pontos alargada como o GEOS faz. `delaunay(a, edges=False,
    tolerance=0)`: os triângulos (ou as arestas) de `ST_DelaunayTriangles`.
  - `cluster(a, distance, min_points=1)`: a coluna `cluster` (0, 1, ... na ordem da primeira feição de cada grupo; -1
    para o ruído), o DBSCAN do `ST_ClusterDBSCAN` sobre a distância entre geometrias (uma feição central tem
    `min_points` feições a até `distance`, ela inclusa; a de borda fica com o primeiro grupo que a alcança, na ordem
    das feições). Com `min_points=1` é o `ST_ClusterWithin` (as feições ligadas por distâncias até `distance`). Sem
    k-means aqui: as coordenadas vão ao `stats.kmeans` das tabelas.
  - Nós `vector.voronoi`, `vector.delaunay`, `vector.cluster`.
- **L5 / A7. Referência linear, linha mais curta e ponto mais próximo.**
  - `points_along(a, spacing=None, n=None, start=0.0)`: pontos ao longo de cada linha a cada `spacing` (ou `n`
    pontos igualmente espaçados), com os atributos da linha e a coluna `measure` (a distância ao longo dela);
    `ST_LineInterpolatePoints`, QGIS "Points along geometry".
  - `substring(a, start, end, normalized=False)` (`ST_LineSubstring`) e `split_lines(a, length)` (trechos de até
    `length`, QGIS "Split lines by maximum length", a coluna `part`).
  - `locate(a, lines)`: para cada feição de `a`, a linha mais próxima (`line`, o índice), a posição ao longo dela
    (`measure`, `ST_LineLocatePoint` em unidades de comprimento) e a distância (`distance`).
  - `shortest_line(a, b)`: a linha de cada feição de `a` à mais próxima de `b` (`ST_ShortestLine`), com os atributos
    de `a`, os de `b` (sufixo) e `distance`. `closest_point(a, b, max_distance=None)`: os pontos de `a` levados ao
    ponto mais próximo de `b` (`ST_ClosestPoint`; ocorrências ajustadas ao rio), os longe demais como estão, a coluna
    `moved` (a distância andada).
  - Nós `vector.points_along`, `vector.substring`, `vector.split_lines`, `vector.locate`, `vector.shortest_line`,
    `vector.closest_point`.
- **L6 / A8. Suavizar e simplificar uma cobertura.**
  - `smooth(a, iterations=1, preserve_ends=True)`: o Chaikin do `ST_ChaikinSmoothing` (cada segmento cortado a 1/4 e
    3/4, os anéis fechados mantidos fechados), o mesmo algoritmo escrito na biblioteca (numpy) e no aplicativo
    (Rust), bit a bit. É o que tira os degraus do `vector.polygonize`.
  - `coverage_simplify(a, tolerance, preserve_boundary=False)`: a simplificação de uma cobertura sem abrir buracos
    nem sobreposições entre vizinhos (`ST_CoverageSimplify`, GEOS 3.12+, `GEOSCoverageSimplifyVW_r`; um erro claro
    com GEOS mais antigo), e `coverage_validate(a, gap_width=0)` (`ST_CoverageInvalidEdges`: as arestas que não
    batem, como linhas, e a coluna `valid`).
  - Nós `vector.smooth`, `vector.coverage_simplify`, `vector.coverage_validate`.

**Feito (2026-11-23), tudo.** Na biblioteca (terraflux-py, branch `vector` em `terraflux-py-vector`, sobre a 1.62.0,
ainda fora do main da biblioteca e sem release): `tf.vector` com os predicados, `select`, `relate`, `count`,
`distance`, `join`, `nearest`, a família do overlay, `union`, `clip`, `dissolve`, `buffer`, a geometria, `voronoi`,
`delaunay`, `cluster`, a referência linear, `smooth` e as coberturas (167 testes). No aplicativo (main, sem push):
A1 a A4 e A6 a A8 pelo GEOS direto (`crates/gdal/src/geos*`, STRtree e predicados preparados), cada nó comparado com
`tf.vector` por uma referência em `tests/data/reference/vector_*.json` (bit a bit nos casos de teste, com GEOS 3.14.1
no app e 3.13.1 na referência); a calculadora com `within(parks)`, `dwithin`, `count` e `distance`; o Python
exportado chama `tf.vector`; o assunto "Vector geometry" no Builder; o template das ferramentas de vetor com
`vector.nearest` e `vector.select_by_location`; A5: o tutorial em duas partes `tutorials/vector-analysis/` com a
tabela de equivalência, `python/api/vector.md` e a seção da calculadora. Ficou para depois:
  - a release: primeiro a biblioteca (fundir `vector` no main dela, release), depois o submódulo e o `ops.json`,
    `python_api.json` refeitos;
  - ~~os dez predicados de `tf.vector` são criados por `_predicate_function`: o griffe não vê docstrings, por isso
    estão à mão em `python/api/vector.md` (melhor virar `def` com docstring)~~ **Feito (2026-11-24)**: dez `def`
    com docstring própria (terraflux-py efddfbe), e um `:::` de cada em `python/api/vector.md`;
  - ~~os polígonos do `vector.polygonize` não são uma cobertura válida do GEOS (o canto de um cai no meio da aresta
    do vizinho): um nó que "node" a camada (shapely `node` + `polygonize`) ou um `vector.polygonize` que já dê os
    vértices partilhados deixaria usar `vector.coverage_simplify` direto~~ **Feito (2026-11-24)**:
    `vector.coverage_node` (abaixo);
  - o Delaunay do GEOS 3.13 e 3.14 escolhe diagonais diferentes com quatro pontos cocirculares.

### Uma cobertura a partir do `vector.polygonize` (pedido de 2026-11-23)

Polígonos do `vector.polygonize`: não formam uma cobertura válida para o GEOS, porque o canto de um polígono cai no
meio da aresta do vizinho, sem vértice ali. As dicas agora mandam validar antes. Para usar
`vector.coverage_simplify` direto, falta um nó que acrescente esses vértices partilhados.

**Feito (2026-11-24).** `tf.vector.coverage_node(a, tolerance=0)` (terraflux-py 08ef82d, na branch `vector`) e o nó
`vector.coverage_node`: cada vértice de outra feição que cai numa aresta, estritamente entre as pontas, entra nela
(exatamente sobre ela por padrão; a até `tolerance` para uma camada reprojetada), na ordem ao longo da aresta. Não é o
`node` + `polygonize` do shapely: esse refaz os polígonos (outra ordem, os atributos por um ponto dentro, os anéis
começando em outro vértice), e este só acrescenta vértices, com as feições, a ordem, os atributos e os anéis como
estavam e a coluna `added`; a mesma aritmética na biblioteca (numpy, o `STRtree` do shapely) e no app (Rust, o
`rstar`), bit a bit (`tests/data/reference/vector_cover.json`, com uma camada mais ruidosa e a escada reprojetada).
O teste `polygonized_patches_noded_are_a_valid_coverage` passa o `vector.polygonize` do app (4 e 8 vizinhos) pelo
nó e pelo `vector.coverage_validate`. Num mapa de 18.501 polígonos, 1 s na biblioteca; de 7.127 válidos para todos.
Na release: o submódulo na biblioteca com o `coverage_node` (o Python exportado o chama).
