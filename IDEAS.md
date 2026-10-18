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

**Fase 0: protótipo antes de código.** Desenhar à mão a vista documento (fase 2) de dois ou três modelos grandes
reais (um CLUE-S completo, uma priorização com zonas, um SWAT com calibração) para ver se ela dá conta: a ordem
das etapas, o que cabe num passo, onde a miniatura ajuda e onde atrapalha.

**Fase 1: o porquê, além do quê (formato).** **Feito** (Unreleased): os campos no nó e no modelo, as ferramentas
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

**Fase 2: a vista documento (a vista principal do usuário final).**
- **O que ela mostra.** O mesmo grafo como uma seção de métodos de artigo, com as etapas em ordem topológica:
  dados → preparo → modelo → validação → resultados. Cada passo traz:
  - a nota e o porquê;
  - os parâmetros em linguagem natural, tirados da descrição do operador ("alcance de 30 células", e não
    `range = 30`);
  - as expressões do `map.calc` com os nomes ligados aos nós de origem;
  - uma miniatura do resultado, tirada do cache do runner.
- **Edição.** Clicar num passo abre o nó. Os parâmetros podem ser mudados ali mesmo, e o modelo roda de novo como
  no builder.
- **Exportação** como relatório (HTML e PDF): o registro auditável de um processamento.
- O canvas vira a vista avançada, para quem quiser os fios.

**Fase 3: revisão das mudanças do agente.** **Feito** (Unreleased): o painel Changes, aberto pelas mudanças do
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

**Fase 4: hierarquia e zoom semântico.**
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

**Fase 6: parâmetros como painel.**
- As entradas `$nome` do modelo ganham tipo, faixa, unidade e texto de ajuda (no formato, como os demais campos
  opcionais). Viram um formulário simples, com controles deslizantes e escolhas, para o usuário mudar um cenário
  sem tocar no grafo.
- O painel se liga aos cenários e à calibração do `batch_view`: salvar um conjunto de parâmetros como cenário e
  comparar cenários lado a lado (mapas, tabelas, diferenças).

**Fase 7: rastreio de uma célula.**
- Clicar num pixel do resultado mostra a cadeia de valores que o produziu, por exemplo: "esta célula virou cidade
  porque a adequação era 0,71, tinha 4 vizinhos urbanos e o limiar no passo 12 era 0,50".
- **Implementação.** Percorrer o grafo para trás a partir do nó mostrado e ler o valor da célula na saída em cache
  de cada nó de origem. Nos laços, usar os passos coletados (com uma opção de coletar todos ao rastrear). Para o
  `map.calc`, mostrar a expressão com os valores substituídos. Operadores de vizinhança e globais (`nbsum`,
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
2. A troca no macOS e no zip/tar.gz, o repositório apt.

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
  passo.
