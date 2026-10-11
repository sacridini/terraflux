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
  - Instaladores a fazer: Windows (MSI ou NSIS, com o redistribuível), depois macOS e Linux.

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
e o potencial de estimadores do scikit-learn (hoje exportado como raster de previsões).

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
  resto: Zonation, Marxan/ProxIRR/Ferrier, o compilador do prioritizr (MILP), prioriactions, o planejamento do
  plangea.
- **Próximos:** MSPA por tiles para rasters maiores que a memória; mapa de fundo de satélite (GDAL XYZ/WMS);
  vetores (OGR) para rasterizar e medir distâncias; instaladores com o GDAL; os outros kernels C++ já prontos (reclassificação, zonal, distância, métricas de paisagem,
  FAD, SPA), depois reprojeção (PROJ embutido e o warp do landschaft) e vetores.

Pendências gerais, depois das fases:
- regiões e demandas regionais, CLUMondo, FUTURES e MOLUSCE no Terraflux;
- reprojeção (PROJ), vetores e distâncias com um clique;
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
