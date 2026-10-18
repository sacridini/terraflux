# A vista documento: protótipo (fase 0) e decisões

Registro de 2026-10-18. A fase 0 de "O modelo como registro do que um agente fez" (`IDEAS.md`): antes de escrever a
vista documento (fase 2), desenhar à mão o documento que ela daria para modelos reais e ver onde ele falha. Os
protótipos estão em inglês, a língua da interface. As decisões no fim são a especificação da fase 2.

Três modelos, de formas diferentes:

1. **Um CLUE-S completo, com dados reais** (28 nós): leitura e preparo de arquivos, um laço, validação. É o caso
   de 40 a 60 nós em miniatura, com o que os templates não têm (reprojeção, vetores, validação contra um mapa
   observado).
2. **"A species in a warmer climate"** (o template, 21 nós): muitos ramos paralelos e quatro `view.plot`.
3. **"Restoration plan (plangea)"** (o template, 15 nós): três espécies preparadas igual, nós sem nota.

---

## Protótipo 1: CLUE-S com dados reais

O modelo imaginado: os mapas de uso de 2000, 2010 e 2020 de um município, as estradas, os rios, o relevo e as
unidades de conservação; CLUE-S calibrado em 2000–2010, simulado até 2020 e validado contra o mapa de 2020.

> # Deforestation in São Félix do Xingu, 2010–2020 (CLUE-S)
> *CLUE-S calibrated on 2000–2010 and checked against the map of 2020.*
>
> **Request.** "I want to know where the deforestation of 2010–2020 would have gone by the 2000s trends, and how
> well CLUE-S gets it." · **Author.** Maria Silva and an assistant (Qwen3 30B) · **Summary.** …
>
> ## 1. Data
>
> | | Step | What it is |
> |---|---|---|
> | ▣ | **land_2000**, **land_2010**, **land_2020** · read a raster | MapBiomas collection 9, classes 1 forest, 2 pasture, 3 crops, 4 water. `data/mapbiomas_2000.tif`… |
> | ▣ | **dem** · read a raster | Copernicus DEM 30 m. `data/dem.tif` |
> | ◇ | **roads**, **rivers** · read vectors | IBGE's roads and rivers. `data/roads.gpkg`, `data/rivers.gpkg` |
> | ◇ | **protected** · read vectors | Protected areas (CNUC). `data/ucs.gpkg` |
>
> *(three reads of the same kind on one row: a list, not three steps)*
>
> ## 2. Preparation
>
> **2.1 dem_grid** · raster.match — *On the grid of the land maps.*
> The elevation put on **land_2010**'s grid: resampling **bilinear**.
>
> **2.2 slope** · raster.terrain — *Slope in degrees, a driver of where farming goes.*
> Of **dem_grid**; product: **slope**.
>
> **2.3 dist_roads**, **dist_rivers** · vector.distance — *Distance to roads and rivers (m).*
> From **roads** / **rivers**, on **land_2010**'s grid.  [▢ ▢ two thumbnails]
>
> **2.4 restricted** · vector.rasterize — *Protected areas keep their class.*
> **protected** burnt into **land_2010**'s grid: value **1**, all touched **no**.
>
> **2.5 drivers** · stack of **slope**, **dist_roads**, **dist_rivers**, **dem_grid**.
> *(plumbing: a line, no card, no thumbnail)*
>
> **2.6 observed** · stack of **land_2000**, **land_2010**.
>
> ## 3. Model
>
> **3.1 regression** · glm.fit — *Logistic regression of each class on the drivers (lulcc's glmModels).*
> **Why.** The suitability of CLUE-S is a regression per class; lulcc's is the reference the results are checked
> against.
> Of the classes of **land_2010** on **drivers**.
> **Assumptions.** The drivers do not change in 2010–2020.
>
> **3.2 suitability** · glm.predict — *The suitability of each class at every cell.* [▢ thumbnail, layer 1 of 4]
>
> **3.3 demand** · demand.from_maps — *The cells of each class, extrapolated linearly from 2000 and 2010.*
> times: **2000, 2010**; out times: **2010, 2020** (one step of 10 years).
>
> **3.4 rng** · rng.r — *R's generator after set.seed(7).* *(a line)*
>
> **3.5 history** · lulcc.history — *How long each cell has had its class.* *(a line)*
>
> **3.6 simulation** · repeated **once per step of the demand** (from **demand.steps**)
> Carries **landscape** (from **land_2010**) and **history** (from **history**); collects **maps**, **info**.
>
> > **3.6.1 allocate** · lulcc.step — *Every cell takes the class of highest suitability plus an iteration
> > variable, moved until the demand is met.*
> > **Why.** …
> > allocation: method **clue**, scale **0.000005** (lulcc: 0.0005), max iter **5000**;
> > elasticity: **0.5, 0.2, 1.0** (per class); restrictions: **restricted**.
>
> [▢ the last map collected, "step 1 of 1"]
>
> ## 4. Validation
>
> **4.1 agreement** · validate.agreement — *The simulation against the map of 2020: quantity and allocation.*
> Of **simulation.maps** (the last) and **land_2020**, from **land_2010**.
> [table: figure of merit 0.21, producer's accuracy…]
>
> **4.2 similarity** · validate.similarity — *Fuzzy similarity in windows of 1 to 9 cells.* [chart]
>
> ## 5. Results
>
> **Figure 1.** *Simulated land use, 2020* (from **simulation.maps**) [▢ large]
> **Figure 2.** *Observed land use, 2020* (from **land_2020**) [▢ large]
>
> | Output | From | File |
> |---|---|---|
> | maps | simulation.maps | `maps.tif` (u8) |
> | agreement | agreement.table | `agreement.csv` |

### O que o protótipo 1 mostra

- **As cinco etapas cabem**, mas a etapa não sai só do operador: `map.calc` pode ser preparo (uma distância
  reclassificada) ou resultado (a diferença entre dois mapas simulados). Sai do operador **e** da posição no grafo:
  depois do primeiro nó de modelo, o que não é validação é resultado.
- **Leituras iguais viram uma linha**: três `raster.read` de mapas de uso numa tabela de dados, não três cartões.
- **Os nós de encanamento** (`stack.of`, `stack.set`, `stack.select`, `rng.*`, `lulcc.history`) sem nota viram uma
  linha. Com nota, um cartão pequeno sem miniatura.
- **O laço** lê bem como um passo com subpassos recuados: o que leva (`carry`), o que coleta (`collect`) e o número
  de passos (de onde vem). A miniatura é a do último mapa coletado.
- **Os parâmetros** em palavras funcionam com a descrição do operador: "allocation: method clue, scale 0.000005".
  Mostrar o padrão ao lado quando o valor difere dele ("lulcc: 0.0005" só existe na nota; o padrão vem da descrição,
  "(default …)"). Os objetos aninhados viram "chave valor, chave valor".
- **A validação** pede tabelas e gráficos, não miniaturas de mapa: a saída de tabela aparece como as primeiras
  linhas.

---

## Protótipo 2: "A species in a warmer climate"

> # A species in a warmer climate
> *A species distribution model explained and projected: …*
>
> ## 1. Data
>
> | | Step | What it is |
> |---|---|---|
> | ▢ | **field_t** · Gaussian field | rows 60, columns 80, range **14 cells**, seed 21 |
> | ▢ | **field_r** · Gaussian field | range **22 cells**, seed 22 |
>
> ## 2. Preparation
>
> **2.1 temperature** · map.calc — *The mean temperature, from 14 to 26 °C.*
> `14 + f * 12` where **f** = **field_t**
>
> **2.2 rainfall** · map.calc — *The rainfall, from 800 to 2400 mm.* `800 + f * 1600`, **f** = **field_r**
>
> **2.3 predictors** · stack of **rainfall**, **temperature**.
>
> **2.4 suitability** · map.calc — *Where the species finds its conditions: about 19 °C and 1600 mm.*
> `exp(-((t - 19) ^ 2) / 3 - ((r - 1600) ^ 2) / 150000)`, **t** = **temperature**, **r** = **rainfall** [▢]
>
> **2.5 presences** · sdm.background — *Presences drawn by the suitability.* n **150**, seed 3
>
> **2.6 data** · sdm.prepare — *A record per cell, 1000 background points, 4 folds of spatial blocks.*
> background **1000**, clean **no**, folds **4**, seed 1
>
> **2.7 warmer** · map.calc — *The temperature in a climate 2 °C warmer.* `t + 2`, **t** = **temperature**
>
> ## 3. Model
>
> **3.1 fit** · sdm.fit — *MaxNet fitted over the folds.* [table: AUC 0.87, TSS …]
>
> ## 4. Validation
>
> **4.1 importance** · sdm.importance — *How much the AUC falls when each predictor is shuffled.* [table]
>
> **4.2 curves** · sdm.response_curves [chart]
>
> ## 5. Results
>
> **5.1 today** · sdm.maxnet_predict — *The suitability today.* → **Figure 1** *Suitability today* [▢]
>
> **5.2 future** · stack.set — the **warmer** temperature in place of today's. *(a line)*
>
> **5.3 tomorrow** · sdm.maxnet_predict — **Figure 2** *Suitability 2 °C warmer* [▢]
>
> **5.4 shift** · map.calc `b - a` (**a** = **today**, **b** = **tomorrow**) — **Figure 3** *Gains and losses* [▢]
>
> **5.5 mess** · sdm.mess — **Figure 4** *MESS (negative: extrapolation)* [▢]

### O que o protótipo 2 mostra

- **A ordem topológica pura embaralha a história.** `warmer` depende só de `temperature` e sairia logo depois dele,
  no meio da preparação do modelo de hoje. A posição no canvas (o autor arruma da esquerda para a direita) conta
  melhor: ordenar pela coluna do canvas (x) e, empatado, pela linha (y), sempre respeitando as dependências. Com a
  ordem por etapas, `warmer` fica na preparação (vem antes do modelo) e é lido como "o dado do futuro", o que está
  certo.
- **Os `view.plot` não são passos**: viram a figura do passo que mostram, com o título como legenda e numeradas.
  São o lugar certo das miniaturas grandes.
- **Os campos sem nota** que só alimentam um `map.calc` com nota (`field_t` → `temperature`) são dados e ficam na
  tabela de dados, com os parâmetros em linha.
- **Os nomes nas expressões** (`f`, `t`, `r`) só fazem sentido ligados ao nó: "**f** = **field_t**" logo abaixo da
  expressão, cada nome clicável.
- **Avaliação de modelo** (`sdm.importance`, `sdm.response_curves`, `sdm.evaluate`) é validação, mesmo sem
  `validate.` no nome: a lista das etapas por operador precisa de famílias, não só de prefixos.

---

## Protótipo 3: "Restoration plan (plangea)"

> # Restoration plan (plangea)
>
> ## 1. Data
>
> | | Step | What it is |
> |---|---|---|
> | ▢ | **field** · Gaussian field | *A smooth surface from 0 to 1 drawn by chance.* range 10 cells |
> | ▢ | **toucan**, **jaguar**, **frog** · Gaussian field | range 15, 20, 6 cells *(no note)* |
>
> ## 2. Preparation
>
> **2.1 forest**, **crops** · map.calc — *The share of forest in each cell; the rest is cropland.*
>
> **2.2 land_cover** · stack of **crops**, **forest**.
>
> **2.3 vegetation** · restoration.original_vegetation — *What the converted land was before.*
> anthropic **crops**; natural **forest**
>
> **2.4 ranges**, **ranges2**, **ranges3** · map.calc — *A species' range: the cells above 0.*
> `t > 0.4` (**t** = **toucan**), `j > 0.5` (**j** = **jaguar**), `f > 0.3` (**f** = **frog**)
>
> **2.5 species** · stack of **frog** (**ranges3**), **jaguar** (**ranges2**), **toucan** (**ranges**).
>
> **2.6 habitat** · sdm.area_of_habitat — *Every species' habitat now and before conversion (km²).*
>
> ## 3. Model
>
> **3.1 plan** · restoration.plan — *The share of every cell to restore, 100 km² in all, …*
> target **100**, cell area **1**, z sd **0.1** → **Figure 1** *Share of each cell restored* [▢]

### O que o protótipo 3 mostra

- **Nós iguais lado a lado** (três `map.calc` de mesma forma, três campos) se leem melhor juntos. Mas juntar
  passos do modelo é arriscado (esconde diferenças). Decisão: juntar só **dados** do mesmo operador numa linha da
  tabela; os demais ficam um a um, curtos quando não têm nota.
- **Um passo sem nota nem porquê** precisa aparecer como tal, discreto ("no note"), sem poluir: é o que o
  `check_model` já avisa.
- **Um modelo pequeno** cabe quase inteiro na preparação. As etapas vazias não aparecem.

---

## Decisões para a fase 2

1. **Uma vista nova, o painel Document**, ao lado do Model. O mesmo grafo, sem mudar o formato.
2. **As etapas.** Cada nó do nível de cima entra numa de cinco etapas, nesta ordem:
   - **Data**: nós sem entradas de outros nós (leituras, `nlm.*`, `data.download`) e as entradas `$nome`;
   - **Preparation**: o que vem antes do primeiro nó de modelo;
   - **Model**: ajustes, simulações, laços, otimizações (por família de operador: `*.fit`, `control.repeat`,
     `prioritize.*`, `restoration.plan`, `lulcc.step`…);
   - **Validation**: `validate.*`, `accuracy.*`, `check.*`, `sdm.evaluate`, `sdm.importance`, `ml.importance`…;
   - **Results**: o que vem depois do modelo e não é validação, os `view.plot` e as escritas.
   Um modelo sem nó de modelo põe tudo o que não é dado nem resultado no preparo. Etapas vazias não aparecem.
   Quando o nível de cima for feito de `control.group` (fase 4), cada grupo é uma seção, com a sua nota.
3. **A ordem.** Dentro de cada etapa, a ordem topológica com os empates resolvidos pela posição no canvas (x, depois
   y), e as etapas na ordem acima. Um nó nunca aparece antes de algo de que ele depende na mesma etapa.
4. **Os tamanhos de um passo.**
   - **Dados**: uma tabela, uma linha por nó (os parâmetros em linha, o caminho do arquivo).
   - **Encanamento** (`stack.of`, `stack.set`, `stack.select`, `stack.rename`, `rng.*`, `lulcc.history`): uma linha.
   - **Os demais**: um cartão com o título (o id e o nome do operador em palavras), a nota, o porquê, as entradas
     ("of **x**"), os parâmetros dados, as referências, as premissas e as alternativas.
   - **Contêineres**: um cartão com os subpassos recuados, o que leva, coleta e dá, e o número de passos.
5. **Os parâmetros em palavras**: "a descrição do parâmetro sem o "(default …)": **valor**", só os parâmetros dados;
   ao lado, "(default x)" quando o valor difere do padrão escrito na descrição. Objetos: "chave valor, …"; listas:
   "a, b, c"; sim e não: "yes", "no".
6. **As expressões do `map.calc`** com os nomes destacados e, abaixo, "**nome** = **nó**" para cada entrada; cada
   nó clicável.
7. **As miniaturas** só onde ajudam: nos cartões de modelo e de resultado, e nos `view.plot` (as figuras,
   numeradas, com o título como legenda). Nos dados e no preparo, pequenas e só para mapas. Nunca no encanamento.
   Uma série: o último passo, com "step k of n". Uma tabela: as primeiras linhas. As miniaturas vêm do cache do
   runner; um nó que não rodou diz "not run".
8. **Edição**: clicar no título de um passo o seleciona no canvas (o painel Node mostra tudo). Os parâmetros dados
   são editáveis no próprio cartão; a edição passa pelo histórico (ctrl+Z) e roda o modelo de novo, como no
   builder.
9. **Exportação**: File › Export report (HTML ou PDF), e um botão no painel. O HTML é um arquivo só, com as
   miniaturas embutidas (PNG em base64), legível sem o Terraflux; o PDF tem o mesmo conteúdo. O relatório também
   sai pela linha de comando (`terraflux report modelo.toml -o relatorio.html`), sem miniaturas ou com
   `--run`.
10. **O que fica para depois**: os parâmetros por frases escritas à mão por operador ("alcance de 30 células" em
    vez de "the range of the autocorrelation, in cells: 30"); a descrição genérica já dá frases legíveis, e as
    frases à mão podem vir aos poucos.

## Como ficou (fase 2)

O painel Document (`crates/app/src/document.rs` monta o documento, `document_view.rs` o desenha,
`report.rs` escreve o HTML e o PDF) segue as decisões acima, com estes ajustes vistos ao implementar:

- **A posição de um nó sem lugar** é a que o canvas lhe dá (`auto_positions`), para a ordem do documento ser a
  do canvas também nos modelos feitos por um agente sem posições.
- **Nenhum passo antes do que ele usa**: depois da classificação por operador e posição, a etapa de um nó é ao
  menos a mais tardia das etapas de que ele depende. Ex.: no template de acurácia, o `map.calc` que rotula a
  amostra sorteada por `accuracy.sample` é validação também.
- **Os grupos** (`control.group`) do nível de cima são seções (fase 4), com a nota do grupo sob o título; o que
  um grupo dá é nomeado pelo nó do corpo que o dá. Os grupos guardam o que o corpo deu (as portas ocultas
  `nó.porta`), então os passos de um grupo têm miniaturas.
- **Um nó sozinho nomeia a sua saída** só quando ele tem uma saída; senão `nó.porta` (um laço que coleta `maps` e
  `info`).
- **Os números** como se escrevem (0.000005, não 5e-6; 1916.4 nas tabelas).
- **O relatório pela linha de comando**: `terraflux report modelo.toml -o relatorio.pdf --run`.
