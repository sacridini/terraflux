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
5. **O Código Florestal completo.** Há o topo de morro (`surface.hilltop`); faltam as APPs dos rios pela largura (os
   rios do `hydro.streams`), as nascentes, as declividades acima de 45°, as bordas de tabuleiro, e o déficit e o
   excedente de Reserva Legal por imóvel do CAR (o SICAR como entrada), como nos trabalhos do Soares-Filho.
6. **Análise de sensibilidade global.** O "Executar várias vezes" tem réplicas, cenários e calibração; faltam os
   desenhos de Morris e de Sobol (as amostras de Saltelli) e os índices (efeitos elementares; primeira ordem e
   totais com intervalos por bootstrap), sobre os lotes e a distribuição que já existem. Referência: o SALib.
7. **Estatística espacial e interpolação.** Nenhum operador interpola: IDW e krigagem ordinária (o variograma
   ajustado) para estações de clima e amostras de solo. Nem mede a autocorrelação local: o I de Moran global, o
   LISA e o Gi* de Getis-Ord (os hotspots de desmatamento), referências no PySAL (`esda`). E a tendência célula a
   célula de uma série de mapas (Mann-Kendall e a inclinação de Sen), muito usada com o NDVI.
8. **O resto do InVEST:** a razão de entrega de nutrientes (NDR), a polinização e a produção sazonal de água, os
   mais pedidos depois dos quatro que já estão. Entram primeiro no landschaft, como os outros.
9. **Dispersão de espécies no tempo.** O SDM de vários períodos com a dispersão célula a célula (como o MigClim) ou
   com uma metapopulação (o modelo de função de incidência), juntando o `sdm.*` e o `conn.*` num laço.

Fora dos operadores:
- **O nó de script em Python**, previsto no plano como componente opcional e isolado (um Python embutido), nunca no
  laço da simulação.
- **Animações das simulações:** os mapas coletados num `control.repeat` exportados em GIF ou MP4, com a legenda e o
  passo.
