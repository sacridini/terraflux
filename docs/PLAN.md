# Plano do Terraflux: sucessor espiritual do Dinamica EGO

## Context

A ideia é criar um **sucessor espiritual do Dinamica EGO**: mais moderno, mais otimizado e mais fácil de
usar, sem ficar simplista. Ele precisa:
- Modelar mudança do uso do solo com autômatos celulares, com uma camada de agentes (ABM) depois.
- Ter GUI em **egui** (Rust) e também rodar headless, distribuído em várias máquinas.
- Escalar até **cenários mundiais a 30 m**.
- Reaproveitar a lógica do `landschaft` (`C:\Users\eduar\Documents\dev\landschaft`, C++17/OpenMP com
  cerca de 150 mil linhas). O `ls.simulate` reproduz lulcc, CLUMondo, r.futures, MOLUSCE, OpenLand e
  diffeR bit a bit; PLUS e Dinamica foram implementados a partir das publicações.
- Ser **objetivo e guiado na preparação dos dados** para quem tem pouca experiência, mas **flexível** para
  quem quer mudar a lógica de funcionamento.

### O que a exploração do landschaft mostrou

- **Kernels C++ reaproveitáveis.** Os de `src/cpp/sim/` (crosstab, WoE, rglm, CLUE-S, CLUMondo, FUTURES,
  Expander/Patcher, CARS, similaridade fuzzy, validação) não dependem de pybind: usam ponteiros crus e
  `std::vector`, e o binding fica isolado em `src/cpp/sim/python.cpp`. Cada módulo é uma biblioteca
  estática (`simcore`, `lsmcore`, `nlmcore`…) no `CMakeLists.txt`.
- **Lógica que existe só em Python e precisa virar nativa:**
  - o laço do `Model.run` (`simulate/_model.py:292`);
  - os laços de Dinamica, PLUS e CLUMondo (`_patches.py`, `_plus.py`, `_clumondo.py`);
  - `Markov`, `AreaDemand`/`Regional`, `Rules`, `Ordered`;
  - o corte de faixas do WoE, a calibração e a maior parte da validação/diffeR;
  - `_dynamic.py`.
- **Bloqueios para distribuir:**
  - O simulate roda inteiro em memória.
  - Todas as alocações ordenam o mapa inteiro (`patches.cpp:174`, `clumondo.cpp:260`, `plus.cpp:90`,
    `futures.cpp:248`, `Ordered`).
  - Os RNGs são sequenciais. Só o `CellRandom(seed, iter, cell)` de `plus.cpp:27` é baseado em contador.
- **Peças úteis para preparar dados:**
  - reprojeção (`io/warp.cpp`, `_reproject.py`);
  - rasterização (`lsm/rasterize.cpp`) e zonal (`io/zonal.cpp`);
  - distâncias e custo (`distance.py`, `cost/`);
  - `_dynamic.py` (`distance_to`, `share_of`);
  - diagnósticos do WoE (Cramér, JIU) em `_woe.py`;
  - leitura em blocos (`lsm/tiled.cpp`, `mspa_tiling.py`).
- **GDAL:** o C++ não usa GDAL; o I/O é por rasterio no Python.
- **Licença:** tudo é GPL-3.0-or-later, com ports de código GPL. O aplicativo também será
  GPL-3.0-or-later.

## Decisões tomadas

- **Linguagens:**
  - Rust no aplicativo: GUI em **egui + wgpu**, motor, I/O, distribuição.
  - Kernels C++ do landschaft via `cxx`.
  - A lógica que hoje está só em Python é portada para C++ dentro do landschaft, validada pelos testes bit a
    bit existentes.
- **Distribuição:** gRPC entre máquinas avulsas e nuvem, com armazenamento compartilhado e template Slurm.
- **Repositório:** novo, com o landschaft como submódulo.
- **ABM:** camada posterior, com espaço reservado na arquitetura.
- **Interfaces:** o **model builder visual e o Python juntos**, como frentes de um mesmo grafo de modelo.
  O Python fica fora do laço de simulação.
- **Instalação para o usuário final (registro de 2026-10-11):** quem instala o Terraflux não precisa saber de
  Rust, C++ ou Python; tudo vem configurado no instalador.
  - O executável não usa Python. Rust e os kernels C++ do landschaft (compilados com `LANDSCHAFT_PYTHON=OFF`)
    estão dentro dele.
  - O GDAL e o PROJ vão na pasta `gdal` ao lado do executável (`tools/bundle-gdal.ps1`), carregados em tempo de
    execução.
  - Falta no Windows limpo o runtime do Visual C++ (`MSVCP140`, `VCRUNTIME140`, `VCRUNTIME140_1`) e o do OpenMP
    (`VCOMP140`): o instalador roda o Visual C++ Redistributable ou leva essas DLLs. Alternativa: compilar com
    `+crt-static` e levar só o `VCOMP140`.
  - O Python só aparece no desenvolvimento (referências dos testes de paridade, já guardadas no repositório) e
    como frente de scripts instalada à parte (`ls.simulate`). Funções do landschaft que só existem em Python são
    portadas para C++ antes de entrar no aplicativo.
  - Se um dia houver Python dentro do aplicativo (um nó de script), ele entra como componente opcional e
    isolado (um Python embutido), nunca no laço da simulação; para estender o aplicativo sem exigir nada do
    usuário, os plugins WebAssembly.
  - Instalador do Windows feito (2026-10-13, `tools/make-installer.ps1`): um MSI do WiX 5 (ferramenta .NET do
    repositório) e um zip portátil, com o GDAL na pasta `gdal` e as DLLs do runtime do Visual C++ e do OpenMP ao
    lado do executável (a implantação local que a Microsoft permite, sem rodar o redistribuível). Faltam macOS e
    Linux, a assinatura do instalador e um ícone.

## Princípios de produto

1. **Um único modelo, várias frentes.** O modelo é um **grafo de operadores tipados**, salvo num formato
   texto versionável. Assistente, editor de nós, Python e arquivo texto produzem e consomem o mesmo grafo.
2. **Fácil sem esconder nada (revelação progressiva).** Toda decisão automática do assistente vira **nós
   explícitos no grafo**, com a justificativa anotada. O iniciante aceita os padrões. O avançado abre o
   grafo e muda qualquer coisa. Não existe "mágica" que não possa ser inspecionada ou sobrescrita.
3. **Padrões corretos por construção.** O app escolhe o método certo pelo tipo de dado: categórico ou
   contínuo, área ou contagem. Ele explica a escolha em uma frase e avisa quando o usuário a contraria.
4. **Diagnóstico antes de rodar.** Um "lint" do modelo aponta problemas antes da simulação. Ver a seção de
   preparação de dados.
5. **Flexível na lógica.** Operadores são extensíveis: álgebra de mapas, submodelos reutilizáveis e
   plugins. Ver a seção de flexibilidade.
6. **O mesmo modelo roda em qualquer escala.** Do notebook ao mundo a 30 m, sem reescrever nada: o motor
   decide como particionar.

## Arquitetura

```
 Assistente (egui) ──┐
 Editor de nós (egui)┼──►  Grafo do modelo (IR tipado, texto)  ──►  Planejador  ──►  Executor local (exato)
 Python (landschaft) ┤                                                         └─►  Executor por tiles (cluster)
 Arquivo .toml ──────┘
```

### 1. Grafo do modelo (o núcleo)

- **Tipos:** mapa categórico, mapa contínuo, tabela, lookup table, matriz de transição, número, máscara,
  região, camada vetorial (e, depois, agentes).
- **Cada operador declara o próprio acesso aos dados.** É isso que permite ao planejador cortar o grafo
  em tiles e sincronizar só onde precisa:
  - `local`: célula a célula;
  - `halo(r)`: vizinhança;
  - `global`: reduções como demanda, histogramas e Markov.
- **Containers no estilo Dinamica:** `Repeat` (passos), `ForEachCategory`, `ForEachRegion`, `Submodel`.
- **Formato texto** (TOML/JSON) com versão de schema e migrações automáticas.
- **Presets como grafos prontos:** `clue_s`, `ordered`, `clumondo`, `futures`, `dinamica`, `plus` e
  `molusce`, a partir de `simulate/presets.py`.

### 2. Dois modos de execução

- **Modo exato** (uma máquina): bit a bit igual ao landschaft e às referências. Usa os mesmos RNGs e as
  mesmas ordenações globais.
- **Modo escalável** (tiles, várias máquinas): estatisticamente equivalente ao exato, e **determinístico
  independentemente do particionamento**. Para isso:
  - RNG por contador (Philox) chaveado por `(seed, passo, transição, id global da célula)`;
  - alocação por **limiar** obtido de histogramas somados entre os workers (comunicação O(bins)), por
    região × transição;
  - halos com a largura do raio de vizinhança;
  - conflitos de manchas entre tiles resolvidos por prioridade determinística.

### 3. Distribuição

- **Um binário** com os subcomandos `terraflux app`, `serve`, `worker` e `run`.
- **Controle:** gRPC (`tonic`).
- **Dados:** os workers leem e escrevem direto em armazenamento compartilhado (S3/MinIO/NFS), em
  **Zarr v3 com sharding** (`zarrs`) e COG (`gdal`). Os halos vão de worker para worker.
- **Ordem:** primeiro paralelismo de **tarefas** (réplicas, cenários, calibração), depois de **domínio**
  (tiles).
- **Dentro de cada máquina:** `rayon` e OpenMP. GPU (`wgpu` compute) depois.
- **Ordem de grandeza mundial:** cerca de 1,6×10¹¹ células de terra, ~165 GB por mapa u8 e ~660 GB por
  mapa de probabilidade f32. Viável com ~16 nós de 256 GB ou com out-of-core.

### 4. Preparação de dados guiada (foco em quem tem pouca experiência)

Um **projeto** tem uma **grade de referência**: CRS, resolução, extensão e máscara de estudo. O fluxo do
assistente:

1. **Importar.** Arrastar rasters e vetores de qualquer CRS e resolução. O app:
   - detecta o tipo (categórico ou contínuo, pela tabela de cores, tipo inteiro e número de valores únicos)
     e pede confirmação;
   - sugere a grade de referência a partir do mapa de uso do solo.
2. **Harmonizar automaticamente para a grade.** O reamostramento correto vem do tipo:
   - categórico: vizinho mais próximo ou moda;
   - contínuo: bilinear ou média;
   - contagem: soma.

   Também cuida de recorte, alinhamento, nodata unificado e máscara. Cada passo vira um nó no grafo.
3. **Legenda e classes.** Editor de legenda e reclassificação (juntar classes) com pré-visualização. Avisa
   classes raras ou ausentes num dos anos.
4. **Mudanças observadas.** A partir de dois ou mais mapas:
   - matriz de transição, análise de intensidade e componentes da diferença;
   - **sugestão das transições a modelar**, ignorando as de área insignificante, com o motivo.
5. **Variáveis explicativas.**
   - Geração com um clique a partir de vetores: distância a estradas, rios e áreas urbanas; distância à
     classe X (dinâmica); declividade a partir do MDE.
   - Diagnóstico de colinearidade (Cramér, JIU, correlação) com sugestão de quais remover.
   - Para o WoE: faixas automáticas e gráficos dos pesos por faixa, editáveis.
6. **Calibração e validação**, que seguem o mesmo padrão:
   - métodos padrão sensatos: similaridade fuzzy em várias janelas, figure of merit, três mapas;
   - gráficos explicados em linguagem simples.

**Lint do modelo antes de rodar:**
- grades desalinhadas;
- nodata inconsistente;
- variável constante;
- transição sem célula de origem;
- demanda impossível (maior que a área disponível);
- variáveis colineares;
- falta de período de validação.

Cada aviso vem com uma ação de correção sugerida.

### 5. Flexibilidade da lógica

- **Álgebra de mapas:** expressões no estilo do *Calculate Map* do Dinamica (condicionais, vizinhança,
  lookup tables), avaliadas por tile. Depois, JIT via Cranelift ou WGSL. Cobre a maior parte da lógica
  customizada **dentro** do laço, e roda em qualquer worker.
- **Submodelos reutilizáveis:** grafos salvos como bibliotecas e usados como um único nó.
- **Operadores nativos plugáveis:**
  - um trait Rust `Operator` (tipos de entrada e saída, acesso, execução por tile) com registro;
  - plugins externos em **WASM**, portáveis e seguros para enviar aos workers.
- **Python fora do laço:** prepara dados, treina potencial (scikit-learn, XGBoost) e analisa resultados.
  O modelo treinado entra no grafo como coeficientes, raster ou ONNX. Um callback Python no laço só é
  aceito em operador marcado "somente local", e o app avisa que esse modelo não distribui.

### 6. Interfaces

- **GUI egui (wgpu):**
  - **Assistente** de projeto, dados e preset.
  - **Editor de nós** (`egui-snarl`), que abre o mesmo grafo, com tipos verificados ao conectar.
  - **Inspeção em cada nó:** clicar mostra o mapa ou a tabela da última execução.
  - **Mapa ao vivo** sobre uma pirâmide de overviews por tiles, com `walkers` para o mapa base opcional.
  - **Gráficos** com `egui_plot`.
- **Python:** o próprio `ls.simulate` do landschaft, acrescido de `Model.save()` e `Model.load()` (grafo)
  e de `ls.simulate.submit(model, cluster=...)`, que devolve `xarray`.
- **CLI:** `run modelo.toml`, `serve`, `worker`.

### 7. ABM (fase posterior)

- **Uma camada sobre o raster,** com `bevy_ecs` standalone num passo síncrono com o CA.
- **Agentes:**
  - macro (países, mercados), que geram a demanda;
  - gestores de parcelas, que modulam potencial e regras;
  - competição por célula no estilo CRAFTY.
- **No grafo:** novos tipos (agentes, parcelas) e operadores.
- **Comportamentos:** Rhai ou WASM.

## Estrutura do repositório novo (`terraflux`)

```
terraflux/                   Cargo workspace, GPL-3.0-or-later
  extern/landschaft          submódulo git
  crates/ls-sys              bridge cxx para simcore/lsmcore/nlmcore (build.rs com crate cmake)
  crates/graph               IR do modelo: tipos, operadores, registro, formato texto, migrações, lint
  crates/ops                 operadores (wrappers dos kernels, álgebra de mapas, containers)
  crates/raster              grade de referência, harmonização, tiles, halos, pirâmides, Zarr/COG
  crates/exec                planejador; executores local (exato) e por tiles (RNG por contador, reduções)
  crates/dist                coordenador e worker (tonic, .proto)
  crates/app                 GUI egui: assistente, editor de nós, mapa, gráficos
  crates/cli                 binário: app | serve | worker | run
```

## Fases

0. **Landschaft:**
   - Portar para C++ (`src/cpp/sim/`) Markov, AreaDemand/Regional, Rules, Ordered, o corte de faixas do
     WoE, o laço do `Model.run` e os laços de Dinamica/PLUS/CLUMondo. O Python vira um wrapper fino.
   - Adicionar a opção CMake `LANDSCHAFT_PYTHON=OFF` (só bibliotecas estáticas).
   - Testes bit a bit inalterados.
1. **Grafo + executor local + CLI:**
   - `crates/graph`, `ops`, `exec` (modo exato) e `ls-sys`;
   - presets como grafos;
   - `run modelo.toml` com GeoTIFF.
2. **Ponte Python:** `Model.save()`/`load()` no landschaft ↔ grafo, com ida e volta sem perda. Também entram
   os operadores que faltaram na fase 1: variáveis dinâmicas, demandas regionais, PLUS/CARS, FUTURES,
   CLUMondo e MOLUSCE.
3. **GUI MVP em egui:**
   - projeto e grade de referência;
   - assistente de preparação de dados (seção 4) com lint;
   - assistente de preset;
   - mapa ao vivo, gráficos e painel de validação;
   - **visualização do grafo** gerado, ainda somente leitura.
4. **Distribuição por tarefas:** réplicas, cenários e calibração em workers headless.
5. **Motor por tiles:**
   - modo escalável, out-of-core e Zarr;
   - benchmarks: estado, bioma, América do Sul e mundo a 30 m.
6. **Editor de nós completo + álgebra de mapas + submodelos + plugins WASM.**
7. **Camada ABM.**

## Andamento

### Fase 0 — feita (landschaft 1.43.0, 2026-10-10)

Portados para C++ (`src/cpp/sim/`), com o Python chamando-os e os testes bit a bit inalterados:
- **Regras:** `rules.cpp` (matriz de conversões com os códigos do lulcc, conversões automáticas, vizinhança).
- **Alocação ordenada:** `ordered.cpp`.
- **Demanda e Markov:** `demand.cpp` (`approxExtrap`, `roundSum`, matriz por contagens, raiz por `log()`/`exp()`
  de matriz do Eigen, projeção).
- **Manchas observadas:** `observed.cpp` (sem `scipy.ndimage`).
- **WoE:** faixas, pesos e probabilidades em `woe.cpp`.

A opção `LANDSCHAFT_PYTHON=OFF` compila e instala só as bibliotecas estáticas. `test_simulate_kernels.py`
guarda o código numpy anterior como referência (74 testes).

Desvio do plano: o laço de passos (`Model.run`, laços do Dinamica/PLUS/CLUMondo) **não** foi para C++. No
Python ele continua como estava; no Terraflux ele é o grafo (`control.repeat` + operadores de um passo). Uma
terceira cópia do laço em C++ não traria nada.

### Fase 1 — feita (Terraflux 0.1.0, 2026-10-10)

- **Crates:** `graph`, `ops`, `exec`, `raster`, `ls-sys` e `cli`. O landschaft entra como submódulo SSH (o
  repositório é privado).
- **Paridade bit a bit com o landschaft**, e portanto com o lulcc:
  - Dinamica em Sibuyan: Markov, pesos, manchas e os mapas dos 14 passos;
  - CLUE-S: vinheta, todas as regras com histórico e máscara, vizinhança;
  - Ordered: estocástico, determinístico, regras com vizinhança.
- **CLI:** `run`, `check`, `ops`, `new dinamica|clue|ordered` e `convert`.

Desvios do plano:
- **E/S de GeoTIFF em Rust puro** (`terraflux-raster`) em vez do crate `gdal`. A máquina não tem as bibliotecas
  de desenvolvimento do GDAL, e assim o aplicativo instala sem DLLs externas. Reprojeção e Zarr ficam para as
  fases 3 e 5.
- **Presets da fase 1:** Dinamica, CLUE-S e Ordered. Ficaram para a fase 2: FUTURES, PLUS/CARS, CLUMondo,
  MOLUSCE, as variáveis dinâmicas (`distance_to`, `share_of`) e as demandas regionais.

### Fase 2 — feita (Terraflux 0.2.0 e landschaft 1.44.0, 2026-10-10)

- **No landschaft:** `Model.save`, `Model.load` e `S.terraflux`. As peças ajustadas vão para o grafo como
  literais (`woe.weights`, `glm.coefficients`, `markov.matrix`, `demand.table`, `patches.settings`) e os dados
  em GeoTIFF float64.
- **No Terraflux, operadores novos:**
  - variáveis dinâmicas (`dynamic.distance_to`, `dynamic.share_of`, `stack.set`);
  - PLUS/CARS (`plus.demand`, `cars.step`);
  - `stack.classes`.
- **Testes:**
  - no landschaft, `run`, `save` → `load` → `run` e `terraflux run` dão os mesmos bits (Dinamica, CLUE-S com
    regras, Ordered, dinâmicas, CARS, potencial em raster);
  - no Terraflux, modelos gravados pelo landschaft batem com os mapas dele.

Pendências movidas para depois da fase 3: demandas regionais, CLUMondo, FUTURES, MOLUSCE, distâncias de custo,
e o potencial de estimadores do scikit-learn (hoje exportado como raster de previsões). Feitas no branch
`simulate-bridge` (ver "A ponte do ls.simulate completa").

### Fase 3 — feita (Terraflux 0.3.0, 2026-10-10)

- **Aplicativo** (`terraflux app`, egui sobre wgpu):
  - assistente de novo modelo com verificação dos dados e reamostragem dos drivers desalinhados;
  - execução em segundo plano com os mapas chegando passo a passo, e botão de parar;
  - abas de gráficos de área, grafo do modelo (com as notas), texto do modelo e validação (figure of merit e
    similaridade fuzzy).
- **Lint dos dados** (também no `terraflux check`), cada problema com o que fazer.
- **Reamostragem** na mesma projeção (`terraflux resample`).
- **Testes da interface** renderizados fora da tela (egui_kittest), com capturas em `docs/images`.

Pendências da preparação de dados, para depois do motor distribuído:
- reprojeção entre CRSs (precisa do PROJ);
- leitura de vetores (estradas, rios) para gerar distâncias com um clique;
- editor de legenda e reclassificação;
- ajuste das faixas do WoE com gráficos editáveis;
- assistente para a análise das mudanças (matriz e intensidade).

### Fase 4 — feita (Terraflux 0.4.0, 2026-10-10)

- **Crate `terraflux-dist` (gRPC com tonic):**
  - coordenador com fila, blobs por SHA-256, registro de workers e reenvio por heartbeat (3 tentativas);
  - workers headless só com conexão de saída e cache por hash;
  - clientes de lote: réplicas, cenários em TOML, calibração por grade ou hipercubo latino, avaliada no worker
    (figure of merit, concordância, similaridade fuzzy) e com a média sobre os seeds.
- **CLI:** `serve`, `worker`, `workers`, `submit` e `calibrate`. Sem `--coordinator`, tudo roda num cluster
  local dentro do próprio processo.
- **Substituições por tarefa** no grafo (`Model::apply`).
- **Documentação:** `docs/cluster.md` e template Slurm.
- **Testes:**
  - réplicas no worker dão os mesmos bytes que localmente;
  - a calibração acha o parâmetro que gerou o mapa "observado";
  - a tarefa de um worker que some roda em outro;
  - cache compartilhado sem corrida.
- **Segurança:** ainda sem autenticação nem TLS. O `serve` escuta em 127.0.0.1 por padrão e a documentação
  recomenda rede confiável ou túnel SSH.

### Fase 5 — motor por tiles feito, numa máquina ou em várias (Terraflux 0.5.0, 2026-10-10)

- **Crate `terraflux-engine` (`terraflux run --tiled`):** o modelo do Dinamica EGO em tiles lidos, calculados e
  escritos um a um, em paralelo. As camadas dos passos ficam em memória ou em disco (`--disk`, tiles f32 com
  LZ4 e cache dos tiles decodificados, `TERRAFLUX_CACHE_MB`).
  - **Ajustes por redução:** Markov e pesos de evidência são somados entre os tiles e saem bit a bit iguais aos
    do motor exato; as manchas observadas são medidas dentro de cada tile.
  - **RNG por contador** (hash SplitMix de semente, passo, transição e célula).
  - **Alocação por limiar:** as sementes do Expander e do Patcher são as células cuja chave
    (Efraimidis–Spirakis, `u^(1/p)`) passa um limiar. Os limiares de poda de todas as transições saem dos
    mesmos histogramas somados entre os tiles; as maiores chaves são coletadas uma vez por fase, e o limiar de
    cada rodada é uma seleção exata. As manchas crescem até `--radius` células da semente, com halo de 2R+2; a
    célula disputada fica com a chave maior. Cada transição para de ajustar quando atinge a demanda, e o
    excesso é cortado pela pontuação exata.
  - Cada célula só avalia as transições da sua classe; cada janela de um arquivo de drivers é decodificada uma
    vez para todas as bandas.
- **Passes como dados** (`terraflux_engine::pass`): cada passada sobre os tiles é uma `Task` que devolve uma
  `Part`, e as partes se juntam em qualquer ordem no mesmo resultado. Um `Runner` executa as passadas: `Local`
  (threads desta máquina) ou `terraflux_dist::tiles::Remote`.
- **Motor por tiles distribuído** (`--coordinator`): cada passada vira um job do coordenador, com trechos
  contíguos de tiles como tarefas (4 por worker). Os workers leem os mapas e escrevem as camadas no
  armazenamento compartilhado, e só as somas (histogramas, contagens, chaves) passam pelo coordenador. O
  coordenador ganhou `Wait` e `Forget`.
- **GeoTIFF por janelas** (`Reader`, `Reader::reopen`) e escrita de tiles em qualquer ordem (`TiledWriter`).
- **Threads:** nenhum comando usa todos os núcleos. O padrão é todos menos um (`--threads`,
  `TERRAFLUX_THREADS`), no pool do rayon, no OpenMP dos kernels do landschaft e nos workers do cluster local.
  Os testes rodam dois de cada vez, com quatro threads cada (`.cargo/config.toml`).
- **Testes:**
  - Markov e WoE iguais aos do motor exato com qualquer tamanho de tile;
  - mesmos mapas com qualquer particionamento, para os mesmos ajustes;
  - demanda atendida (2%);
  - equivalência estatística com o motor exato em Sibuyan: similaridade fuzzy de 0,690 contra 0,682 e áreas
    por classe a cerca de uma célula;
  - os mesmos mapas com os tiles em 2 workers (trechos de qualquer tamanho) e nas threads desta máquina;
  - mapas em memória não vão para workers.
- **Benchmark** (Sibuyan repetido 32 × 32 vezes: 8192 × 6912 = 56,6 milhões de células, 10 transições, 3
  passos, tiles de 1024, camadas em disco, **4 threads**): 318 s no total, com 43 s de ajustes e cerca de 92 s
  por passo (33 s de probabilidades do WoE). Pico de memória de 3,3 GB, dos quais até 2 GB são de cache de
  tiles. A demanda de cada transição foi atendida exatamente. A primeira versão levava 533 s; a melhora veio
  das passadas em lote, da coleta única das chaves, da convergência por transição, da leitura única das bandas
  e do cache dos buckets do WoE.

Falta para fechar a fase 5:
- Zarr v3 e COG;
- os outros modelos (CLUE-S, ordenado, CARS) e as variáveis dinâmicas por tiles;
- probabilidades do WoE mais rápidas por tile (hoje o kernel do motor exato, bit a bit);
- benchmarks de estado, bioma, América do Sul e mundo, e de escalabilidade em várias máquinas.

### Fase 6 — feita (Terraflux 0.6.0, 2026-10-10)

- **Editor de nós** (aba Graph do aplicativo):
  - adicionar operadores e ligar portas arrastando, com cada ligação verificada pelos tipos, ciclos e portas;
  - mover nós (a posição fica no modelo, `pos`), renomear com as referências acompanhando, e apagar;
  - editar valores e configurações;
  - entrar no corpo de laços e grupos, vendo à esquerda o que vem de fora;
  - salvar o modelo como escrito.

  As operações ficam em `terraflux_app::editor`, testadas sem interface.
- **Álgebra de mapas** (`map.calc`, como o Calculate Map do Dinamica):
  - expressões sobre entradas de qualquer nome: mapas, números e camadas de pilhas (`stack.layer`);
  - aritmética, comparações, lógica, `if then else` e funções;
  - vizinhanças `nbsum nbmean nbmin nbmax nbcount`;
  - nulos que se propagam.
- **Grupos e submodelos:**
  - `control.group` roda o corpo uma vez, com entradas `@nome` e saídas `body.outputs`;
  - `control.submodel` usa outro arquivo de modelo como um nó e é expandido em grupo antes de rodar. As
    entradas de arquivo dele viram entradas do modelo de fora.
- **Plugins WebAssembly** (`plugin.wasm`, com wasmi):
  - uma função de qualquer linguagem chamada por célula;
  - sandbox sem imports e com orçamento de instruções;
  - em lotes, o módulo vai para os workers por hash.
- **Documentação:** `docs/modelling.md`.
- **Verificação da fase:** um modelo com um submodelo de álgebra de mapas e um plugin dá nos workers os mesmos
  bytes que localmente.

Pendências:
- álgebra de mapas, grupos e plugins no motor por tiles (hoje só o modelo clássico do Dinamica);
- JIT da álgebra de mapas (Cranelift ou WGSL);
- operadores nativos plugáveis em Rust carregados em tempo de execução;
- biblioteca de submodelos no aplicativo.

### Fase 7 — feita (Terraflux 0.7.0, 2026-10-10)

- **Agentes no mesmo grafo:** o tipo `Agents` é uma população em colunas de atributos, carregada nos laços
  como o mapa.
  - `agents.table`: agentes macro (países, mercados) escritos no modelo.
  - `agents.parcels`: um gestor por parcela, com o tamanho e as médias de mapas sobre ela.
  - `agents.update`: o comportamento, em regras `atributo = expressão` (as expressões da álgebra de mapas).
  - `agents.map`: um atributo pintado nas parcelas, como potencial, restrição ou multiplicador.
  - `agents.summary`: contagem, somas e médias para coletar.
- **CRAFTY** (`crafty.allocate`):
  - tipos funcionais de agentes produzem serviços a partir dos capitais das células;
  - a demanda não atendida dá valor aos serviços;
  - há abandono abaixo do limiar e tomada da célula pelo tipo mais competitivo acima do limiar de cessão do
    ocupante;
  - o RNG é por contador.
- **Teste de acoplamento:**
  - a população cresce e gera a demanda;
  - os gestores subsidiam a fertilidade da sua parcela;
  - o CRAFTY aloca, a oferta acompanha a demanda e o subsídio atrai agricultores.
- **Decisão diferente do plano:** em vez de `bevy_ecs`, uma tabela colunar de atributos. Os modelos de agentes
  de uso do solo são populações de atributos numéricos processadas em lote, e a tabela é mais simples, mais
  rápida e se integra ao grafo e à álgebra de mapas sem outra dependência. Os comportamentos usam as
  expressões da álgebra de mapas em vez de Rhai; plugins WebAssembly por agente ficam como extensão.

### Builder como interface do landschaft — começo feito (2026-10-10)

- **Decisão:** o Terraflux vira a interface gráfica do landschaft sem copiar o C++. O landschaft continua a
  fonte única dos kernels (submódulo), com os testes bit a bit; cada função entra por um shim (`ls-sys`) e um
  operador. A lógica que só existe em Python é portada para C++ dentro do landschaft. O Python fica fino, não é
  abandonado: é a frente de scripts e a prova de paridade.
- **O aplicativo abre no builder:** biblioteca de operadores por assunto, abrir e salvar raster, operadores
  ligados ao nó selecionado, e o que o nó dá mostrado sob o canvas.
- **Execução incremental:** cache por nó (impressão digital do nó, das dependências, das entradas e dos
  arquivos lidos), execução até um nó e de novo depois de cada edição; o que não mudou vem do cache.
- **Responsividade (meta: 60 fps):** nada demorado roda num frame. Leitura dos dados, legendas e coloração
  dos mapas em threads; o mapa anterior fica na tela enquanto o próximo é colorido. Com um mapa de
  2048 x 1728 e o MSPA rodando, os frames ficam abaixo de 4 ms.
- **Operadores:** `raster.read`, `raster.write` e `landscape.mspa` (bit a bit com o executável do GTB).
- **Canvas na GPU para diagramas enormes:** a cena (posições, portas, conexões e uma grade espacial) é montada
  só quando o grafo muda, em tempo linear; caixas, portas e curvas são desenhadas pela GPU com instancing
  (pipelines wgpu dentro do render pass do egui); a câmera tem pan e zoom; o texto é escrito só nos nós
  visíveis e legíveis; arrastar um nó move só ele na cena. Com 10 mil nós: frame de 0,1 ms, arrasto de 0,5 ms,
  edição de cerca de 30 ms. A verificação do modelo e a ordem de execução deixaram de ser quadráticas.
- **Decisão:** não migrar para Dear ImGui/C++. A lentidão era algorítmica (layout cúbico refeito a cada
  frame), e o caminho de alto desempenho (GPU, cena em cache) existe dentro do egui.
- **GDAL embutido (decisão de 2026-10-11):** o Terraflux passa a ser distribuído com o GDAL, como o QGIS, para
  ter todos os formatos, reprojeção, reamostragem, vetores e mapas de fundo. O GDAL é carregado em tempo de
  execução (crate `terraflux-gdal`, só as funções da API C usadas): a compilação não depende dele, o instalador
  leva as DLLs na pasta `gdal` ao lado do executável, e sem ele o GeoTIFF segue pelo leitor nativo (mais rápido
  e exato). Já feitos: abrir qualquer formato, `raster.reproject`, `raster.match`, `raster.clip`.
- **Rasters de qualquer tamanho:** abrir lê só o cabeçalho; o visualizador é uma pirâmide de tiles feita em
  threads. Um raster de 7,3 bilhões de células aparece em 0,4 s. Analisar rasters assim (o MSPA de 44 anos a
  30 m) pede o processamento por tiles com halo, que é o próximo passo dessa frente.
- **Interface em painéis** (`egui_dock`), destacáveis em janelas próprias para outro monitor; biblioteca nas
  categorias do landschaft.
- **O resto do GuidosToolbox (2026-10-11):** `landscape.spa`, `landscape.accounting`, `fragmentation.fad`,
  `fragmentation.fad_multiscale` e `fragmentation.fos`, pelo mesmo caminho do MSPA (shim e operador, o C++ do
  landschaft sem cópia). O que o Python do FOS fazia em volta dos kernels (códigos, manchas, ECA, médias em
  precisão simples como o GTB) foi para o operador em Rust. Testados bit a bit contra as saídas do GTB que o
  landschaft guarda: SPA em 24 casos, Accounting em 35, FAD-MS em 6 mapas e FOS em 13. O viewer dá a cada produto
  as cores e os nomes do GTB, e a legenda de uma densidade sai nas suas classes.
- **Mais do GuidosToolbox (2026-10-11):** contagion, clustering, entropia (do mapa e em janela), landscape
  mosaic, GraySpatCon, parcellation, contortion, distância euclidiana e as mudanças simples, morfológica e do
  FOS. O mosaico de 103 classes é testado contra as saídas do GTB; os outros contra o landschaft (os mapas bit a
  bit; os números dos relatórios até o último bit, que muda com as otimizações do compilador). As paletas do
  GTB vêm do landschaft por um gerador (`tools/make_gtb_palettes.py`).
- **Métricas de paisagem (2026-10-11):** `metrics.patch`, `metrics.class`, `metrics.landscape` e
  `metrics.window`, pelo C++ do landschaft (a mesma `Landscape` que o Python usa). As 256 métricas do catálogo
  vêm do landschaft por um gerador. Testadas contra o landscapemetrics do R: todos os valores que o landschaft
  guarda (14 rasters, 5 conjuntos de opções, mais de 100 mil valores) e as janelas móveis. O visualizador de
  tabelas virou um componente (ordenar, filtrar, barras).
- **Priorização e restauração — começo (2026-10-11):** o landschaft ganhou a opção `LANDSCHAFT_MILP`, que compila
  o `optcore` e o HiGHS sem Python, e o Terraflux liga o `priocore`, o `restcore` e o `optcore`. Primeiro
  sub-bloco: as entradas do plangea (`restoration.intactness`, `intactness_gradient`, `original_vegetation` e
  `sdm.area_of_habitat`), bit a bit com as saídas do plangea. A causa de diferenças no último bit em testes
  anteriores era o serde_json, que sem `float_roundtrip` lia um decimal no double vizinho; corrigido. A ordem do
  resto: Zonation (feito: `prioritize.zonation`, os 41 casos do grupo core bit a bit com o z5; depois, em 0.11.0,
  as condições, a retenção, a conectividade (a convolução por FFT em blocos portada para Rust), os impactos e o
  pós-processamento, com os dados quantizados do C++ mantidos entre os passos: os 65 casos do landschaft),
  Marxan e ProxIRR (feitos: `prioritize.marxan`, os 8 casos iguais ao
  Marxan 4.0.6; `prioritize.proxirr`, bit a bit com o proxirr do R; o Ferrier fica com o compilador, pois precisa
  do problema), o compilador do prioritizr (MILP; D1 feito:
  `prioritize.optimize` com unidades de raster, os objetivos sem termos espaciais, os alvos, pesos, decisões e
  travas; os modelos bit a bit com o compile() do prioritizr nos 20 casos de raster, e o ótimo do Gurobi alcançado
  pelo HiGHS; D2: penalidades de fronteira e lineares, restrição linear e de vizinhos, os 25 casos de raster bit a
  bit; `prioritize.evaluate` (eval_*: resumo, representação, cobertura, Ferrier, raridade, importância por
  substituição e por ranking) bit a bit; D3: unidades de tabela (colunas de feições ou o rij do Marxan), matrizes de
  fronteira, conectividade (simétrica e assimétrica) e adjacência, a avaliação de tabelas, os portfólios (cuts com o
  corte do prioritizr corrigido, top, gap, shuffle), a contiguidade e a contiguidade por feição (a formulação exata
  do landschaft, com cortes por fluxo máximo), os limites manuais, os objetivos filogenéticos e a calibração
  (`prioritize.calibrate`: blended, hierárquica, Pareto, Cohon, TOPSIS e joelho), tudo bit a bit com o compile() do
  prioritizr ou com os seus ótimos; as zonas de manejo (unidades, alvos, objetivos, orçamentos e pesos por zona,
  travas e limites por zona, alocação obrigatória, termos espaciais com matrizes de zonas, contiguidade por grupo
  de zonas, avaliação geral e por zona; 37 casos bit a bit, com os motores de zonas do landschaft); faltam as
  unidades de polígonos (entraram em 0.13.0 com os vetores, os modelos bit a bit com o compile() do prioritizr);
  o risco de extinção como objetivo (`min_extinction_risk`, a
  extensão do landschaft) entrou em 0.12.0, os modelos bit a bit com o compile() do landschaft); E feito: prioriactions (`prioritize.actions`, o
  modelo bit a bit, os ótimos e as figuras dos 6 casos, pelo C++ do landschaft); F feito: o planejamento do plangea
  (`restoration.plan`: risco de extinção e benefícios lineares, metas de área por paisagem ou região, orçamento,
  o método exato por decomposição simplicial com o limite de Frank-Wolfe, os passos do plangea, o plano uniforme,
  conservação e conversão, curvas de trade-off com o gradiente, mudanças projetadas; a redução do risco com a
  incerteza de z; `restoration.ecosystem_areas` e `ecosystem_targets`), com os números do plangea nos casos que o
  landschaft guarda. Depois (0.10.0, 2026-10-12): o benefício de integridade no planejador (linearizado, bit a
  bit com o gradiente do landschaft e refeito entre os passos do plangea; ou exato, por minorantes côncavos
  sucessivos com o limite das secantes do C++ do landschaft), o custo mínimo (regula falsi no orçamento) e a
  comparação de pesos (o post_choose_weights do plangea), iguais aos planos do landschaft
  (`tools/make_plangea_plan_reference.py`).
- **Vetores (0.13.0, 2026-10-12):** um tipo `Vector` no grafo (geometrias em 2-D, atributos, sistema de
  coordenadas), lidos e escritos pelo OGR do GDAL carregado; `vector.rasterize` (polígonos pelo port em C++ do
  rasterizador do GDAL que o landschaft tem, as mesmas células; linhas pelo GDAL; pontos pela célula),
  `vector.zonal` e `vector.exact_sum` (a biblioteca `iocore` do landschaft, agora ligada), `vector.distance`;
  unidades de planejamento em polígonos no prioritizr (matrizes de fronteira e adjacência das geometrias); o
  viewer desenha os vetores ao lado dos atributos.
- **O fim do GuidosToolbox (0.14.0, 2026-10-12):** GTN (`networks.gtn_components`, `gtn_importance`,
  `conefor`; a rotulagem, os contatos e o dPC em Rust), o Restoration Planner (`restoration.status`,
  `resistance`, `cost_distance`, `least_cost_path`, `corridor`, `isochrone`; o ggeo, o afinamento ancorado e a
  distância quase euclidiana do IDL pelo C++ do landschaft), `distance.influence_zones` e `objects_proximity`,
  `change.fad` (as somas do numpy reproduzidas em pares), o heatmap do mosaico e `change.mosaic`, e os
  relatórios de texto (fos.txt, spaN.txt, acc.txt). Tudo igual ao landschaft: os mapas bit a bit, os relatórios
  como texto.
- **Amostras, séries e correlação do `lsm` (0.15.0, 2026-10-12):** `metrics.sample` (o sample_lsm e o
  scale_sample do landscapemetrics: o recorte do terra, as células do rasterizador do GDAL com ALL_TOUCHED, os
  senos x87 do R nos círculos, a área geodésica pelo GeographicLib do GDAL; os 9 casos do R que o landschaft
  guarda, bit a bit), `vector.grid_zones`, `metrics.series` (o SpatioTemporalAnalysis, igual ao landschaft) e
  `metrics.correlation` (o cor do R pelo C++ do landschaft, bit a bit). O código EPSG de um GeoTIFF de projeção
  própria deixou de ser o do datum.
- **Assinaturas do `lsm` (Unreleased, 2026-10-13):** `metrics.signatures` (a tabela do SpatialSignatureAnalysis:
  zonas ou camadas por métricas de paisagem e de classe, bit a bit), `metrics.pca`, `metrics.nmf`,
  `metrics.cluster` e `metrics.clustergram`. O k-means sorteia do gerador do numpy (PCG64 e SeedSequence, os
  mesmos números), as ligações são as do scipy (cadeia de vizinhos, árvore geradora mínima, o algoritmo genérico
  para centroide e mediana, o corte maxclust); a SVD é de Jacobi, a 1e-11 da do LAPACK. Os mesmos grupos do
  landschaft (`tools/make_signature_reference.py`).
- **Rasters maiores que a memória (Unreleased, 2026-10-13):** as métricas por blocos de linhas
  (`block_rows` em `metrics.patch`, `metrics.class` e `metrics.landscape`: o `lsm::TiledLandscape` do landschaft
  pela ponte do ls-sys; as mesmas tabelas da memória bit a bit, de mapas e de rasters em disco) e o MSPA Tiling do
  GTB (`landscape.mspa_tiled`: janelas lidas e tiles TIFF gravados por linha de tiles; os mesmos mapas do
  landschaft, `tools/make_mspa_tiled_reference.py`).
- **Instalador do Windows (Unreleased, 2026-10-13):** `tools/make-installer.ps1` gera o MSI (WiX 5) e o zip
  com o executável, o GDAL e os runtimes do Visual C++ e do OpenMP; o MSI extraído roda um modelo que lê um
  `.asc` pelo GDAL embutido com o PATH só do Windows.
- **Gráficos de tabelas (Unreleased, 2026-10-13):** barras, linhas e pontos de várias colunas de uma vez (um
  painel cada), as linhas por grupo (classe, zona, cluster) numa legenda só; o clustergram das médias do
  `metrics.clustergram`; a matriz de correlação em mapa de calor (`crates/app/src/charts.rs`).
- **Pendências do prioritizr (Unreleased, 2026-10-13):** unidades em polígonos com zonas de manejo
  (`polygon_zone_units`: custos por zona, as feições de cada zona ou contribuições, somas exatas; o compile() do
  prioritizr bit a bit e o ótimo) e as unidades H3 (`prioritize.h3_units`: o h3o, port do H3 em Rust; os mesmos
  hexágonos e custos do landschaft, bit a bit).
- **Restos dos blocos de 0.17.0 (2026-10-13):** `vector.join` (os grupos das zonas como atributos, e por
  `vector.rasterize` um mapa deles), box plots por grupo e o círculo de correlações do PCA nos gráficos, e as
  manchas de um raster em disco gravadas bloco a bloco (`patches_path`, o `RowsWriter`).
- **Vetores, viewer e builder (Unreleased, 2026-10-13):** vetores sobre os mapas e sozinhos em tiles feitos por
  threads de um R-tree (250 mil polígonos indexados em 60 ms, a camada inteira desenhada em 90 ms fora da thread da
  tela), `vector.extract`, `vector.buffer`, `vector.clip`, `vector.dissolve` e `vector.join`; a rampa pelos
  percentis 2–98, a escala logarítmica, a paleta à mão por saída, o produto gravado com o mapa
  (`landschaft_product`); o builder pelo teclado, templates do Dinamica EGO e do CLUE-S e os do usuário. A ponte
  do ls.simulate (o branch da outra sessão) integrada.
- **Métricas e vetores fora da memória (Unreleased, 2026-10-13):** `metrics.window` por faixas com o meio lado
  da janela em volta (os mesmos mapas bit a bit; de rasters em disco, um GeoTIFF de uma banda por métrica),
  `metrics.sample` janela a janela, `metrics.series` e `metrics.signatures` de rasters em disco; `vector.zonal` por
  faixas (as zonas que cabem numa faixa pelo C++ do landschaft nela, uma zona maior corrida por corrida, somada
  como o C++ soma), `vector.extract` pelas faixas que têm pontos (o extract por blocos do landschaft) e
  `vector.rasterize` num GeoTIFF faixa a faixa. Tudo igual à memória, bit a bit (`crates/ops/tests/
  tiled_metrics.rs`, `vector_tiled.rs`); a exceção são as linhas do GDAL com `all_touched` que passam pelos cantos
  das células, que dependem de onde a faixa começa, como nos swaths do próprio GDAL.
- **Preparação com um clique (Unreleased, 2026-10-13):** o painel Prepare, ao lado do do nó: a grade de referência
  do modelo (a do uso do solo sugerida, ou a do nó selecionado; ao abrir um modelo, a que os passos dele usam), o
  dado do nó selecionado ao lado dela (sistema de coordenadas, células, extensão e o que difere; classes, valores
  ou contagens, que decidem a reamostragem) e os passos: para a grade (`raster.match`), reprojetar, declividade,
  distância à classe; dos vetores, a distância, rasterizar e estatísticas zonais; todos os rasters fora da grade
  de uma vez. Cada passo é um nó de um operador que já existia, com a nota do porquê
  (`crates/app/src/prepare.rs`, `prepare_view.rs`; testes em `crates/app/tests/prepare.rs` e `ui.rs`).
- **Análise das mudanças (0.25.0, 2026-10-13):** `change.crosstab`, `change.transitions`, `change.intensity`
  (Aldwaik & Pontius, com a estacionariedade), `change.components` (quantidade, troca e deslocamento),
  `change.count` e `change.suggest` (as transições a modelar, com o motivo), bit a bit com o landschaft; o
  assistente no painel Prepare.
- **Editor de legenda e WoE editável (0.26.0, 2026-10-13):** `raster.legend` (nomes e cores das classes,
  levados com o mapa nos metadados da grade e no GeoTIFF), o editor no viewer; o WoE em barras por transição e
  variável, intervalos juntados ou divididos e pesos à mão (`ranges` por transição e `weights` no `woe.fit`).
- **Circuitos — começo (0.27.0, 2026-10-13):** `circuit.omniscape` e `circuit.advanced`, bit a bit com o C++ do
  landschaft (a construção sem AVX2), nos casos das referências dele.
- **Circuitscape com pontos focais (0.28.0, 2026-10-13):** `circuit.pairwise` e `circuit.one_to_all` (e
  all-to-one), bit a bit com o landschaft.
- **Superfícies — métricas (0.29.0, 2026-10-13):** `surface.metrics` (as 27 do geodiv) e `surface.window`, bit
  a bit com o landschaft.
- **Superfícies — terreno (0.30.0, 2026-10-13):** terrain, TPI, TRI, VRM, SRR, dissecção, curvaturas, desvio
  esférico, HSP, TRASP, HLI, planos, picos e vales, curva de Abbott-Firestone e AACF, bit a bit com o landschaft.
- **Distâncias na grade — caminhos de menor custo (0.31.0, 2026-10-13):** `cost.transition` (as funções do
  gdistance, 4, 8 ou 16 direções, simétrica ou não; uma pilha de uma camada por direção, o layout nos metadados
  da grade), `cost.geo_correction`, `cost.acc_cost`, `cost.distance` e `cost.shortest_path`, bit a bit com o
  landschaft numa grade projetada, numa em graus e numa global que dá a volta.
- **Distâncias na grade — passeios aleatórios (0.32.0, 2026-10-13):** `cost.commute_distance`, `cost.passage`
  (com e sem theta), `cost.rsp_distance`, `cost.normalize`, `cost.sum_reciprocal`, `cost.combine`, `cost.math` e
  `cost.to_map`, bit a bit com o landschaft (o método 2 do RSP, pelo SparseLU do Eigen, varia nos últimos bits de
  uma execução para outra no próprio landschaft; testado a 1e-9).
- **Distâncias na grade — o resto (0.33.0, 2026-10-13):** `cost.barriers`, `cost.areas` (conjuntos de
  transições numa pilha), `cost.mahal` (o `solve` do R reescrito em Rust) e `cost.path_inc`, bit a bit com o
  landschaft. O módulo `cost` está todo no Terraflux.
- **Biodiversidade — basedata, condições e índices (0.34.0, 2026-10-13):** o tipo basedata, `bio.table`,
  `bio.points` e `bio.spatial` (o Perl das condições espaciais, sua compilação, os tipos de resultado e a
  reciclagem das vizinhanças portados do Python do landschaft para Rust; o motor de índices em C++), bit a bit
  com o landschaft, índices escalares e em lista.
- **Biodiversidade — árvores (0.35.0, 2026-10-13):** o tipo árvore, `bio.tree` (o leitor de Newick e Nexus de
  Biodiverse portado), os índices filogenéticos no `bio.spatial` (a poda da árvore e os totais portados) e as
  listas de índices em JSON, bit a bit com o landschaft.
- **Biodiversidade — randomizações (0.36.0, 2026-10-13):** `bio.randomise` (todas as funções do Biodiverse,
  subconjuntos, rótulos fixos, árvores embaralhadas; contadores, p-ranks, z-scores e CANAPE), bit a bit com o
  landschaft.
- **Biodiversidade — agrupamentos (0.37.0, 2026-10-13):** `bio.cluster` e `bio.region_grower` (o
  link_recalculate por um callback do C++ para o Rust; o acabamento da árvore e o corte em k portados), bit a
  bit com o landschaft.
- **Biodiversidade — propriedades e matrizes (0.38.0, 2026-10-13):** `bio.properties` e `bio.matrix` (o tipo
  matriz de rótulos), os índices de propriedades e de matriz, as propriedades de grupo nas randomizações, bit a
  bit com o landschaft. O módulo `bio` está no Terraflux, menos as importações de rasters, hexágonos e H3, as
  condições de shapefiles e os callables do Python.
- **Incerteza (0.39.0, 2026-10-13):** `uncertainty.sum_sd`, `uncertainty.product_sd`, `uncertainty.interval`,
  `uncertainty.delta` e `uncertainty.monte_carlo` (as expressões do map.calc; o ziggurat normal do numpy, o
  ndtri do cephes, as somas e quantis do numpy), bit a bit com o landschaft.
- **Lotes (0.40.0, 2026-10-13):** `batch.run` (qualquer operador em muitos rasters, em paralelo, com os nomes
  do landschaft e do GTB e um JSON por entrada).
- **Circuitos — redes e pares incluídos (0.41.0, 2026-10-13):** `circuit.network_advanced`,
  `circuit.network_pairwise` e os arquivos de pares incluídos/excluídos do Circuitscape nos modos em raster e de
  rede, bit a bit com o landschaft.
- **Circuitos — arquivos INI (0.42.0, 2026-10-13):** `circuit.circuitscape_ini` e `circuit.omniscape_ini`
  (os leitores do Circuitscape e do Omniscape portados; as saídas escritas como eles as nomeiam), bit a bit com
  o landschaft nos 51 casos de teste do Circuitscape em raster e nos INI do Omniscape; os arquivos byte a byte.
  O módulo de circuitos está todo no Terraflux.
- **Modelos neutros (0.43.0, 2026-10-13):** os outros 16 do `nlm` — `nlm.random`, `nlm.percolation`,
  `nlm.planar_gradient`, `nlm.edge_gradient`, `nlm.distance_gradient`, `nlm.curds`, `nlm.mpd`, `nlm.neigh`,
  `nlm.random_rectangular_cluster`, `nlm.mosaic_field`, `nlm.mosaic_tess`, `nlm.mosaic_gibbs` e os utilitários
  `nlm.classify`, `nlm.binarize`, `nlm.merge` e `nlm.rescale` —, bit a bit com o landschaft. Os mosaicos de
  Voronoi dão a cada germe o seu valor (o `nlmr_compat=False` do landschaft): a ordem das células do GEOS que o
  NLMR segue não está portada.
- **Grafos de habitat (0.44.0, 2026-10-13):** o `HabitatGraph` do `conn` (tipo novo) — `conn.graph`,
  `conn.read_conefor`, `conn.write_conefor`, `conn.overall`, `conn.node_importance` (com restauração e
  betweenness), `conn.link_importance`, `conn.link_change`, `conn.components` e `conn.centrality` —, bit a bit
  com o landschaft, menos o que ele calcula com o BLAS, as exponenciais SIMD ou o LAPACK do numpy (a rounding).
- **Grafos de patches em raster (0.45.0, 2026-10-13):** `conn.nodes`, `conn.distances` (centróides, bordas,
  menor custo e tempo de comutação), `conn.graph_from_raster` e `conn.to_map`, bit a bit com o landschaft.
  Faltam do `conn`: os nós e distâncias de vetores, o ProtConn, o SAMC, `focal_nodes` e as grades.
- **Mudança do ECA (0.46.0, 2026-10-13):** `conn.eca_change` (uma série de grafos), bit a bit com o landschaft.
- **Grafos de vetores (0.47.0, 2026-10-13):** `conn.nodes` e `conn.distances` em feições,
  `conn.graph_from_vector`, `conn.to_vector` e `conn.focal_nodes`, com o GEOS pelo GDAL, bit a bit com o
  landschaft.
- **ProtConn e grades (0.48.0, 2026-10-13):** `conn.protconn`, `conn.protconn_regions`, `conn.make_grid`,
  `conn.connectivity_grid` e `conn.eca_change_grid`, bit a bit com o landschaft.
- **SAMC e ProtConn em raster (0.49.0, 2026-10-13):** `conn.settlement_probabilities`, `conn.graph_from_samc`
  (o LU esparso do Eigen no lugar do SuperLU: a rounding) e `conn.protconn_raster` (bit a bit). O módulo `conn`
  está todo no Terraflux.
- **SDM — extrapolação (0.50.0, 2026-10-13):** `sdm.mess`, `sdm.mop` e `sdm.exdet`.
- **SDM — avaliação (0.51.0, 2026-10-13):** `sdm.evaluate`, `sdm.boyce`, `sdm.threshold` e `sdm.omission_rate`.
- **SDM — MaxNet (0.52.0, 2026-10-13):** `sdm.maxnet` e `sdm.maxnet_predict` (tipo `SdmModel`), bit a bit.
- **SDM — registros e background (0.53.0, 2026-10-13):** `sdm.thin`, `sdm.one_per_cell` e `sdm.background`,
  bit a bit.
- **SDM — partições (0.54.0, 2026-10-13):** `sdm.random_folds`, `sdm.jackknife_folds`, `sdm.quadrant_folds`,
  `sdm.checkerboard_folds`, `sdm.spatial_blocks`, `sdm.spatial_clusters` e `sdm.buffer_folds`, bit a bit.
- **SDM — fluxo de modelagem (0.55.0, 2026-10-13):** `sdm.dataset`, `sdm.fit` (validação cruzada),
  `sdm.response_curves` e `sdm.importance`, bit a bit.
- **SDM — preditores (0.56.0, 2026-10-13):** `sdm.vif`, `sdm.select_predictors` e `sdm.autocorrelation_range`,
  bit a bit.
- **SDM — limpeza de registros (0.57.0, 2026-10-14):** `sdm.clean_occurrences` (os testes do CoordinateCleaner)
  e `sdm.reference_data`; as tabelas do CoordinateCleaner dentro do executável, as camadas do Natural Earth
  baixadas no primeiro uso (o cache fixado por SHA-256); as mesmas marcas do landschaft.
- **SDM — área acessível (0.58.0, 2026-10-14):** `sdm.accessible_area` (buffer, envoltória convexa, regiões),
  o mesmo polígono do landschaft (as áreas a 1e-9; o GEOS do GDAL não dá os mesmos vértices).
- **SDM — preparação (0.59.0, 2026-10-14):** `sdm.prepare` (cada passo o nó do seu operador, uma semente), bit a
  bit com o landschaft; o template "Species distribution model" no builder. O módulo `sdm` está no Terraflux,
  menos o `download_gbif` (a API do GBIF) e os modelos do scikit-learn do `fit` (só o MaxNet).
- **Próximos (2026-10-13):** os instaladores de macOS e Linux;
  `vector.exact_sum` e `vector.distance` fora da memória.

- **A ponte do ls.simulate completa (branch `simulate-bridge`, 2026-10-13):** todo modelo do `ls.simulate` vai pelo
  `Model.save` e roda no Terraflux com os mesmos bits. Estimadores e outros potenciais sem forma fechada como
  previsões (`stack.classes`); demandas regionais (`demand.regional`, tipo `Regional`, nos passos do CLUE-S,
  ordenado, CARS e Expander/Patcher); `distance_to` com custo (o `accCost` do `costcore`); MOLUSCE (`rng.numpy`,
  `molusce.total`, `molusce.step`); CLUMondo (`clumondo.ages`, `clumondo.step`; o ano sem solução encerra o laço
  pelo `until` novo do `control.repeat`); FUTURES com enchentes (`futures.simulation`). O que era numpy no
  landschaft foi para o C++ dele (`sim/molusce.cpp`, a vizinhança e as preferências do CLUMondo). Os casos de
  `tests/data/bridge` saem do `tools/make_reference.py bridge`.

Pendências gerais, depois das fases:
- reprojeção (PROJ), vetores e distâncias com um clique (feito: o painel Prepare);
- editor de legenda, gráficos editáveis do WoE e assistente de análise das mudanças;
- motor por tiles para os outros modelos, variáveis dinâmicas, álgebra de mapas e agentes, além de Zarr/COG;
- plugins WebAssembly para o comportamento de agentes;
- autenticação e TLS no coordenador.

## Verificação

- **Fase 0:** `pytest` do landschaft passa sem alterar tolerâncias.
- **Fase 1:** cada preset rodado pela CLI e por `ls.simulate.run` com o mesmo seed dá mapas idênticos byte
  a byte.
- **Fase 2:**
  - Ida e volta do grafo: `Model → save → load → Model` dá o mesmo resultado.
  - Um modelo salvo pelo Python roda na CLI com saída idêntica.
- **Fase 3:**
  - Com os dados do exemplo `pie_landuse` (`ls.simulate.example_data`) em CRS e resoluções diferentes, o
    assistente harmoniza, simula e valida de ponta a ponta.
  - O lint detecta casos de teste com erros plantados.
- **Fase 4:** N réplicas distribuídas são idênticas a N rodadas locais.
- **Fase 5:**
  - Invariância ao particionamento: o mesmo seed com 1, 4 e 16 workers e tiles diferentes dá mapas
    idênticos.
  - Equivalência estatística com o modo exato: demanda exata, tamanhos de manchas, fuzzy similarity e
    figure of merit.
  - Escalabilidade fraca e forte.
- **Fase 6:** um modelo customizado com álgebra de mapas e submodelo roda local e distribuído com o mesmo
  resultado.
