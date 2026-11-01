//! What a step does with its settings, in a sentence written by hand for the operators models use most ("A
//! Gaussian random field of 100 × 100 cells: autocorrelation range 10 cells…"), shown in the document and the
//! report with the settings listed.
//!
//! A phrase is a template: `{name}` is a setting's value in words (the one given, else the operator's default),
//! `<port>` where an input comes from (a node's name, a model input's), and `[…]` a part said only when every
//! setting and input it names is given.

use terraflux_graph::{Node, OpSignature};

use crate::document::From;
use crate::methods::Lang;

/// The phrases, by operator.
const PHRASES: &[(&str, &str)] = &[
    ("raster.read", "Reads the rasters of {path}[, band {band}][, with {nodata} as no data]."),
    ("raster.write", "Writes <value> to {path}[ as {type}]."),
    ("vector.read", "Reads the features of {path}[, layer {layer}]."),
    ("vector.write", "Writes the features of <vector> to {path}[, layer {layer}]."),
    ("table.read", "Reads the table {path}."),
    ("move.read_tracks", "Reads the animal tracks of {path}."),
    ("view.plot", "Shows[ <value>][ <features>][ <table>][ over <under>][ as \"{title}\"]."),
    ("nlm.random", "A random map of {rows} × {columns} cells[ of side {resolution}], seed {seed}."),
    (
        "nlm.gaussian_field",
        "A Gaussian random field of {rows} × {columns} cells[ of side {resolution}]: autocorrelation range \
         {range} cells, variance {variance}, nugget {nugget}; seed {seed}.",
    ),
    (
        "nlm.random_cluster",
        "Random clusters on {rows} × {columns} cells[ of side {resolution}], a share {p} of the cells in clusters\
         [ ({neighbourhood} neighbours)][, the classes weighted {weights}]; seed {seed}.",
    ),
    (
        "nlm.fbm",
        "Fractional Brownian motion on {rows} × {columns} cells[ of side {resolution}], fractal dimension \
         {dimension}; seed {seed}.",
    ),
    (
        "nlm.planar_gradient",
        "A planar gradient on {rows} × {columns} cells[ of side {resolution}][, facing {direction}°]; seed {seed}.",
    ),
    (
        "nlm.distance_gradient",
        "A gradient of the distance to a rectangle[ ({origin})] on {rows} × {columns} cells; seed {seed}.",
    ),
    ("nlm.binarize", "<map> split into habitat and matrix, a share {breaks} of it habitat."),
    ("raster.reclassify", "<map> with its values reclassified: {rules}."),
    ("raster.reproject", "<raster> reprojected[ to {crs}][, cells of side {resolution}][, by {resampling}]."),
    ("raster.resample", "<raster> resampled to cells of side {resolution}[, by {resampling}]."),
    ("markov.fit", "The Markov chain of the changes from <t0> to <t1>[, over <steps> steps]."),
    ("glm.fit", "A logistic regression of each class of <landscape> on <drivers>[ (formulas {formulas})]."),
    ("glm.predict", "The probability of each class by the regression <model>, on <drivers>[ and <landscape>]."),
    (
        "woe.fit",
        "The weights of evidence of each change from <t0> to <t1> by <drivers>[, ranges by increments of \
         {increment}][, {categorical} taken as categories].",
    ),
    ("woe.probability", "The probability of each change by the weights <model>, on <landscape> and <drivers>."),
    ("demand.from_maps", "The area of each class over time, from <maps>[ dated {times}][, for {out_times}]."),
    ("dinamica.step", "A step of Dinamica's allocation of the changes <rates> on <landscape>, by <probability>."),
    ("lulcc.step", "A step of the allocation of <demand> on <landscape>, by <suitability>."),
    (
        "patches.observe",
        "The patches of the changes from <t0> to <t1>: how much they expand and their isometry[ ({isometry})].",
    ),
    ("metrics.class", "The metrics of each class of <map>[: {metrics}][, patches of {directions} neighbours]."),
    (
        "metrics.landscape",
        "The metrics of the whole landscape <map>[: {metrics}][, patches of {directions} neighbours].",
    ),
    ("metrics.patch", "The metrics of each patch of <map>[: {metrics}][, patches of {directions} neighbours]."),
    (
        "landscape.mspa",
        "MSPA's patterns of <map>[, its foreground {foreground}]: connected by {connectivity} neighbours, the \
         edges {edge_width} cell wide.",
    ),
    ("validate.agreement", "How well <simulated> agrees with <observed>, both from <start>."),
    ("validate.kappa", "The kappa of <simulated> against <reference>."),
    (
        "sdm.fit",
        "A species distribution model fitted to <records>[ on {predictors}][, its features {classes}]\
         [, regularisation {regmult}], output {type}.",
    ),
    (
        "sdm.area_of_habitat",
        "Each species' area of habitat within <ranges> on <land_cover>[, natural classes {natural}].",
    ),
    (
        "services.carbon",
        "The carbon stored in <lulc> by the pools <pools>[, and its change to <future>][ valued at {price} a ton].",
    ),
    (
        "prioritize.optimize",
        "The planning units chosen[ for <features>][ at the cost <cost>], the objective {objective}[ within a budget \
         of {budget}].",
    ),
    ("raster.calc", "Maps computed cell by cell from its inputs by a script."),
    ("stack.of", "Its inputs as the layers of a stack."),
    ("table.of", "A table written in the model, its columns {columns}."),
    ("table.write", "Writes <table> to {path}."),
    ("table.filter", "The rows of <table> where {expression}."),
    ("check.assert", "A check: {expression}[ ({message})]."),
    ("conn.to_map", "The values of <table> painted on the patches of <graph>[, the column {column}]."),
    (
        "conn.graph_from_raster",
        "A habitat graph of the patches[ of <map>][ of <vector>][, the habitat classes {habitat}], the distances \
         by {distance}[ over <resistance>][ up to {max_distance}].",
    ),
    ("conn.nodes", "The nodes of the patches[ of <map>][ of <vector>][, the habitat classes {habitat}]."),
    (
        "conn.node_importance",
        "Each node's importance in <graph>[ at a distance {distance}][, probability {probability}].",
    ),
    ("conn.overall", "The connectivity of <graph>[ at a distance {distance}][, probability {probability}]."),
    (
        "dynamic.distance_to",
        "The distance from each cell of <landscape> to the nearest cell[ of the classes {classes}][, over the \
         costs <cost>].",
    ),
    ("sdm.maxnet_predict", "The suitability by the model <model> on <env>[, output {type}]."),
    ("stack.set", "<stack> with <map> as its layer {name}."),
    ("hydro.flow_accumulation", "The flow accumulation of <dem> by {method}[, weighted by <weights>], in {units}."),
    ("hydro.fill", "<dem> with its depressions filled."),
    (
        "hydro.streams",
        "The streams of <accumulation>: the cells of an accumulation of {threshold} or more[, traced on <dem>].",
    ),
    ("rng.r", "R's random number generator, seed <seed>."),
    ("vector.filter", "The features of <vector> where {expression}."),
    ("vector.calc", "The attributes of <vector>[ ({column})] computed from the others by a script."),
    ("vector.extract", "The values of <map> at the points of <vector>, by {method}[, in a radius of {buffer}]."),
    ("vector.zonal", "The statistics of <map> in each polygon of <vector>[: {stats}]."),
    ("vector.points", "Points ({method}) inside the polygons <vector>[, {n} in each][, {spacing} apart]; seed {seed}."),
    ("vector.polygonize", "The patches of <map> as polygons[, {neighbours} neighbours]."),
    ("agents.update", "The agents <agents> updated by the rules: {rules}."),
    ("agents.parcels", "An agent per parcel of <parcels>, with the mean of the other maps in each."),
    ("agents.map", "The attribute {column} of <agents> painted on their parcels <parcels>."),
    ("agents.summary", "The number of agents in <agents> and the sum and mean of each attribute."),
    ("lulcc.history", "The steps each cell of <landscape> has had its class[, from <history>]."),
    (
        "fragmentation.fos",
        "The fragmentation of <map>[, its foreground {foreground}] at a fixed scale: {method} in a window of \
         {window} cells.",
    ),
    (
        "sdm.prepare",
        "The records <occurrences> prepared for a species distribution model on <env>[, the species {species}][, \
         thinned at {thin} m]; seed {seed}.",
    ),
    ("sdm.background", "{n} background points on <env>[ within <area>][, weighted by <bias>]; seed {seed}."),
    ("sdm.clean_occurrences", "The records <occurrences> cleaned of suspicious coordinates (action: {action})."),
    ("sdm.thin", "The records <occurrences> thinned to {distance} m apart, {reps} repetitions; seed {seed}."),
    ("sdm.mess", "The environmental similarity (MESS) of <env> to <reference>."),
    ("surface.terrain", "The {v} of <map>[ in {unit}][, {neighbors} neighbours]."),
    ("raster.terrain", "The {measure} of <raster>[, gdaldem's options {options}]."),
    ("surface.curvature", "The {type} curvature of <map>."),
    ("surface.tpi", "The topographic position index of <map>[, a window of {scale}][, {win}]."),
    (
        "nlm.mosaic_tess",
        "A Voronoi mosaic of random values on {rows} × {columns} cells[ of side {resolution}][, {germs} germs]; \
         seed {seed}.",
    ),
    (
        "nlm.percolation",
        "A random binary map of {rows} × {columns} cells[ of side {resolution}], each cell 1 with probability \
         {prob}; seed {seed}.",
    ),
    ("nlm.classify", "<map> in classes[: {n} by natural breaks][, the shares {weights}][ as in <real>]."),
    ("change.transitions", "The transitions between the maps of <maps>[ dated {times}]."),
    ("change.crosstab", "The transition matrix from <from> to <to>."),
    (
        "change.components",
        "The components of the difference[ from <from>][ to <to>][ along <maps>], the analysis of {analysis}.",
    ),
    ("change.simple", "The change of the foreground[ {foreground}] from <before> to <after>."),
    ("change.morphological", "The morphological change of the foreground[ {foreground}] from <before> to <after>."),
    (
        "change.fad",
        "The change of fragmentation (FAD) from <before> to <after>[, the foreground {foreground}], in windows of \
         {windows} cells.",
    ),
    ("restoration.plan", "A restoration plan by {method}[ within a budget of {budget}][, the target {target}]."),
    ("metrics.signatures", "The spatial signatures of <maps>[ in the zones <plots>]."),
    ("metrics.window", "The landscape metrics[ {metrics}] of <map> in a moving window."),
    ("networks.gtn_components", "The components of the habitat network of the MSPA map <map>."),
    ("circuit.pairwise", "Circuitscape's effective resistance between every pair of the points <points> on <habitat>."),
    (
        "circuit.omniscape",
        "Omniscape's connectivity on <resistance>[ from the sources <source>], in windows of radius {radius} \
         cells[, a target every {block_size} cells].",
    ),
    ("cost.passage", "The passages of a random walk from <origins> to <goals> through <transition>[, theta {theta}]."),
    (
        "prioritize.evaluate",
        "The evaluation of the solution[ <solution>][ <selection>][ for <features>][ at the cost <cost>].",
    ),
    ("prioritize.zonation", "Zonation's priority ranking of <features> by the rule {rule}[ at the cost <cost>]."),
    (
        "prioritize.marxan",
        "Marxan's reserves[ for <features>][ at the cost <cost>][, boundary length modifier {blm}], {num_reps} runs.",
    ),
    (
        "services.pollination",
        "The pollination of crops by wild bees on <lulc>, by the table <biophysical>[ and the farms <farms>].",
    ),
    (
        "services.habitat_quality",
        "The habitat quality of <lulc> under the threats <threat_maps>, by the sensitivities <sensitivity>[, and \
         in the future <future>].",
    ),
    (
        "plus.demand",
        "The cells of each class at each step from <start>[ by the chain <markov>][ by the demand <demand>][ per \
         region <regional>][, {steps} steps].",
    ),
    (
        "landscape.accounting",
        "The objects of the foreground of <map>[ ({foreground})] in size classes, connected by {connectivity} \
         neighbours.",
    ),
    (
        "raster.sieve",
        "<map> with its patches of fewer than {threshold} cells merged into their largest neighbour[, {neighbours} \
         neighbours].",
    ),
    (
        "validate.similarity",
        "The fuzzy similarity of the change in <simulated> and <observed>, both from <start>, in windows of \
         {windows} cells.",
    ),
    (
        "accuracy.sample",
        "A stratified random sample of the cells of <map>[, {n} per stratum][ by <sizes>]; seed {seed}.",
    ),
    ("data.download", "Data downloaded[ from {source}][ ({variable})][ onto the grid of <like>][, cut to <area>]."),
    ("raster.legend", "The names and colours of the classes of <map>."),
    ("markov.matrix", "A Markov chain given in the model[, the classes {classes}], over {steps} steps."),
    ("raster.stats", "A summary of <map>[: {stats}]."),
    ("raster.clip", "<raster> cut to the extent {extent}."),
    ("distance.euclidean", "The Euclidean distance to the edge of the foreground of <map>[ ({foreground})]."),
    ("move.filter_unique", "The tracks <tracks> without duplicated times (criterion {criterion})."),
    (
        "uncertainty.monte_carlo",
        "{n} draws of {expression}, its parameters drawn from normal distributions; seed {seed}.",
    ),
];

/// The phrases in Portuguese, the same placeholders as the English.
const PHRASES_PT: &[(&str, &str)] = &[
    ("raster.read", "Lê os rasters de {path}[, banda {band}][, com {nodata} como sem dado]."),
    ("raster.write", "Grava <value> em {path}[ como {type}]."),
    ("vector.read", "Lê as feições de {path}[, camada {layer}]."),
    ("vector.write", "Grava as feições de <vector> em {path}[, camada {layer}]."),
    ("table.read", "Lê a tabela {path}."),
    ("move.read_tracks", "Lê as trilhas dos animais de {path}."),
    ("view.plot", "Mostra[ <value>][ <features>][ <table>][ sobre <under>][ como \"{title}\"]."),
    ("nlm.random", "Um mapa aleatório de {rows} × {columns} células[ de lado {resolution}], semente {seed}."),
    (
        "nlm.gaussian_field",
        "Um campo aleatório gaussiano de {rows} × {columns} células[ de lado {resolution}]: alcance da \
         autocorrelação de {range} células, variância {variance}, efeito pepita {nugget}; semente {seed}.",
    ),
    (
        "nlm.random_cluster",
        "Agrupamentos aleatórios em {rows} × {columns} células[ de lado {resolution}], uma fração {p} das células \
         em agrupamentos[ ({neighbourhood} vizinhos)][, as classes com pesos {weights}]; semente {seed}.",
    ),
    (
        "nlm.fbm",
        "Movimento browniano fracionário em {rows} × {columns} células[ de lado {resolution}], dimensão fractal \
         {dimension}; semente {seed}.",
    ),
    (
        "nlm.planar_gradient",
        "Um gradiente planar em {rows} × {columns} células[ de lado {resolution}][, voltado para {direction}°]; \
         semente {seed}.",
    ),
    (
        "nlm.distance_gradient",
        "Um gradiente da distância a um retângulo[ ({origin})] em {rows} × {columns} células; semente {seed}.",
    ),
    ("nlm.binarize", "<map> dividido em habitat e matriz, uma fração {breaks} dele habitat."),
    ("raster.reclassify", "<map> com os valores reclassificados: {rules}."),
    ("raster.reproject", "<raster> reprojetado[ para {crs}][, células de lado {resolution}][, por {resampling}]."),
    ("raster.resample", "<raster> reamostrado para células de lado {resolution}[, por {resampling}]."),
    ("markov.fit", "A cadeia de Markov das mudanças de <t0> para <t1>[, em <steps> passos]."),
    ("glm.fit", "Uma regressão logística de cada classe de <landscape> em <drivers>[ (fórmulas {formulas})]."),
    ("glm.predict", "A probabilidade de cada classe pela regressão <model>, em <drivers>[ e <landscape>]."),
    (
        "woe.fit",
        "Os pesos de evidência de cada mudança de <t0> para <t1> por <drivers>[, faixas em incrementos de \
         {increment}][, {categorical} tomadas como categorias].",
    ),
    ("woe.probability", "A probabilidade de cada mudança pelos pesos <model>, em <landscape> e <drivers>."),
    ("demand.from_maps", "A área de cada classe ao longo do tempo, de <maps>[ datados {times}][, para {out_times}]."),
    ("dinamica.step", "Um passo da alocação das mudanças <rates> do Dinamica em <landscape>, pela <probability>."),
    ("lulcc.step", "Um passo da alocação de <demand> em <landscape>, pela <suitability>."),
    (
        "patches.observe",
        "As manchas das mudanças de <t0> para <t1>: quanto elas se expandem e a sua isometria[ ({isometry})].",
    ),
    ("metrics.class", "As métricas de cada classe de <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    ("metrics.landscape", "As métricas da paisagem inteira <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    ("metrics.patch", "As métricas de cada mancha de <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    (
        "landscape.mspa",
        "Os padrões do MSPA de <map>[, o primeiro plano {foreground}]: conectados por {connectivity} vizinhos, as \
         bordas de {edge_width} células de largura.",
    ),
    ("validate.agreement", "Quanto <simulated> concorda com <observed>, os dois a partir de <start>."),
    ("validate.kappa", "O kappa de <simulated> contra <reference>."),
    (
        "sdm.fit",
        "Um modelo de distribuição de espécies ajustado a <records>[ em {predictors}][, as feições {classes}]\
         [, regularização {regmult}], saída {type}.",
    ),
    (
        "sdm.area_of_habitat",
        "A área de habitat de cada espécie dentro de <ranges> em <land_cover>[, as classes naturais {natural}].",
    ),
    (
        "services.carbon",
        "O carbono estocado em <lulc> pelos estoques <pools>[, e a sua mudança para <future>][ a {price} a tonelada].",
    ),
    (
        "prioritize.optimize",
        "As unidades de planejamento escolhidas[ para <features>][ ao custo <cost>], o objetivo {objective}[ num \
         orçamento de {budget}].",
    ),
    ("raster.calc", "Mapas calculados célula a célula a partir das entradas por um script."),
    ("stack.of", "As entradas como as camadas de uma pilha."),
    ("table.of", "Uma tabela escrita no modelo, as colunas {columns}."),
    ("table.write", "Grava <table> em {path}."),
    ("table.filter", "As linhas de <table> onde {expression}."),
    ("check.assert", "Uma verificação: {expression}[ ({message})]."),
    ("conn.to_map", "Os valores de <table> pintados nas manchas de <graph>[, a coluna {column}]."),
    (
        "conn.graph_from_raster",
        "Um grafo de habitat das manchas[ de <map>][ de <vector>][, as classes de habitat {habitat}], as \
         distâncias por {distance}[ sobre <resistance>][ até {max_distance}].",
    ),
    ("conn.nodes", "Os nós das manchas[ de <map>][ de <vector>][, as classes de habitat {habitat}]."),
    (
        "conn.node_importance",
        "A importância de cada nó em <graph>[ à distância {distance}][, probabilidade {probability}].",
    ),
    ("conn.overall", "A conectividade de <graph>[ à distância {distance}][, probabilidade {probability}]."),
    (
        "dynamic.distance_to",
        "A distância de cada célula de <landscape> à célula mais próxima[ das classes {classes}][, pelos custos \
         <cost>].",
    ),
    ("sdm.maxnet_predict", "A adequabilidade pelo modelo <model> em <env>[, saída {type}]."),
    ("stack.set", "<stack> com <map> como a camada {name}."),
    (
        "hydro.flow_accumulation",
        "O fluxo acumulado de <dem> pelo método {method}[, ponderado por <weights>], em {units}.",
    ),
    ("hydro.fill", "<dem> com as depressões preenchidas."),
    (
        "hydro.streams",
        "Os rios de <accumulation>: as células de acumulação de {threshold} ou mais[, traçados em <dem>].",
    ),
    ("rng.r", "O gerador de números aleatórios do R, semente <seed>."),
    ("vector.filter", "As feições de <vector> onde {expression}."),
    ("vector.calc", "Os atributos de <vector>[ ({column})] calculados a partir dos outros por um script."),
    ("vector.extract", "Os valores de <map> nos pontos de <vector>, pelo método {method}[, num raio de {buffer}]."),
    ("vector.zonal", "As estatísticas de <map> em cada polígono de <vector>[: {stats}]."),
    (
        "vector.points",
        "Pontos ({method}) dentro dos polígonos <vector>[, {n} em cada][, a {spacing} de distância]; semente {seed}.",
    ),
    ("vector.polygonize", "As manchas de <map> como polígonos[, {neighbours} vizinhos]."),
    ("agents.update", "Os agentes <agents> atualizados pelas regras: {rules}."),
    ("agents.parcels", "Um agente por parcela de <parcels>, com a média dos outros mapas em cada uma."),
    ("agents.map", "O atributo {column} de <agents> pintado nas suas parcelas <parcels>."),
    ("agents.summary", "O número de agentes em <agents> e a soma e a média de cada atributo."),
    ("lulcc.history", "Há quantos passos cada célula de <landscape> está na sua classe[, de <history>]."),
    (
        "fragmentation.fos",
        "A fragmentação de <map>[, o primeiro plano {foreground}] numa escala fixa: {method} numa janela de \
         {window} células.",
    ),
    (
        "sdm.prepare",
        "Os registros <occurrences> preparados para um modelo de distribuição de espécies em <env>[, a espécie \
         {species}][, rarefeitos a {thin} m]; semente {seed}.",
    ),
    ("sdm.background", "{n} pontos de fundo em <env>[ dentro de <area>][, ponderados por <bias>]; semente {seed}."),
    ("sdm.clean_occurrences", "Os registros <occurrences> limpos de coordenadas suspeitas (ação: {action})."),
    (
        "sdm.thin",
        "Os registros <occurrences> rarefeitos a {distance} m uns dos outros, {reps} repetições; semente {seed}.",
    ),
    ("sdm.mess", "A similaridade ambiental (MESS) de <env> com <reference>."),
    ("surface.terrain", "A medida {v} de <map>[ em {unit}][, {neighbors} vizinhos]."),
    ("raster.terrain", "A medida {measure} de <raster>[, as opções do gdaldem {options}]."),
    ("surface.curvature", "A curvatura {type} de <map>."),
    ("surface.tpi", "O índice de posição topográfica de <map>[, uma janela de {scale}][, {win}]."),
    (
        "nlm.mosaic_tess",
        "Um mosaico de Voronoi de valores aleatórios em {rows} × {columns} células[ de lado {resolution}][, \
         {germs} germes]; semente {seed}.",
    ),
    (
        "nlm.percolation",
        "Um mapa binário aleatório de {rows} × {columns} células[ de lado {resolution}], cada célula 1 com \
         probabilidade {prob}; semente {seed}.",
    ),
    ("nlm.classify", "<map> em classes[: {n} por quebras naturais][, as frações {weights}][ como em <real>]."),
    ("change.transitions", "As transições entre os mapas de <maps>[ datados {times}]."),
    ("change.crosstab", "A matriz de transição de <from> para <to>."),
    (
        "change.components",
        "Os componentes da diferença[ de <from>][ para <to>][ ao longo de <maps>], a análise de {analysis}.",
    ),
    ("change.simple", "A mudança do primeiro plano[ {foreground}] de <before> para <after>."),
    ("change.morphological", "A mudança morfológica do primeiro plano[ {foreground}] de <before> para <after>."),
    (
        "change.fad",
        "A mudança da fragmentação (FAD) de <before> para <after>[, o primeiro plano {foreground}], em janelas de \
         {windows} células.",
    ),
    (
        "restoration.plan",
        "Um plano de restauração pelo método {method}[ num orçamento de {budget}][, a meta {target}].",
    ),
    ("metrics.signatures", "As assinaturas espaciais de <maps>[ nas zonas <plots>]."),
    ("metrics.window", "As métricas da paisagem[ {metrics}] de <map> numa janela móvel."),
    ("networks.gtn_components", "Os componentes da rede de habitat do mapa do MSPA <map>."),
    ("circuit.pairwise", "A resistência efetiva do Circuitscape entre cada par dos pontos <points> em <habitat>."),
    (
        "circuit.omniscape",
        "A conectividade do Omniscape em <resistance>[ das fontes <source>], em janelas de raio {radius} células[, \
         um alvo a cada {block_size} células].",
    ),
    (
        "cost.passage",
        "As passagens de um passeio aleatório de <origins> para <goals> por <transition>[, theta {theta}].",
    ),
    ("prioritize.evaluate", "A avaliação da solução[ <solution>][ <selection>][ para <features>][ ao custo <cost>]."),
    ("prioritize.zonation", "A ordem de prioridade do Zonation de <features> pela regra {rule}[ ao custo <cost>]."),
    (
        "prioritize.marxan",
        "As reservas do Marxan[ para <features>][ ao custo <cost>][, modificador do comprimento de fronteira \
         {blm}], {num_reps} rodadas.",
    ),
    (
        "services.pollination",
        "A polinização das culturas por abelhas silvestres em <lulc>, pela tabela <biophysical>[ e as fazendas \
         <farms>].",
    ),
    (
        "services.habitat_quality",
        "A qualidade do habitat de <lulc> sob as ameaças <threat_maps>, pelas sensibilidades <sensitivity>[, e no \
         futuro <future>].",
    ),
    (
        "plus.demand",
        "As células de cada classe a cada passo a partir de <start>[ pela cadeia <markov>][ pela demanda \
         <demand>][ por região <regional>][, {steps} passos].",
    ),
    (
        "landscape.accounting",
        "Os objetos do primeiro plano de <map>[ ({foreground})] em classes de tamanho, conectados por \
         {connectivity} vizinhos.",
    ),
    (
        "raster.sieve",
        "<map> com as manchas de menos de {threshold} células juntadas à maior vizinha[, {neighbours} vizinhos].",
    ),
    (
        "validate.similarity",
        "A similaridade fuzzy da mudança em <simulated> e <observed>, os dois a partir de <start>, em janelas de \
         {windows} células.",
    ),
    (
        "accuracy.sample",
        "Uma amostra aleatória estratificada das células de <map>[, {n} por estrato][ por <sizes>]; semente {seed}.",
    ),
    ("data.download", "Dados baixados[ de {source}][ ({variable})][ na grade de <like>][, recortados por <area>]."),
    ("raster.legend", "Os nomes e as cores das classes de <map>."),
    ("markov.matrix", "Uma cadeia de Markov dada no modelo[, as classes {classes}], em {steps} passos."),
    ("raster.stats", "Um resumo de <map>[: {stats}]."),
    ("raster.clip", "<raster> recortado na extensão {extent}."),
    ("distance.euclidean", "A distância euclidiana à borda do primeiro plano de <map>[ ({foreground})]."),
    ("move.filter_unique", "As trilhas <tracks> sem tempos duplicados (critério {criterion})."),
    (
        "uncertainty.monte_carlo",
        "{n} sorteios de {expression}, os parâmetros sorteados de distribuições normais; semente {seed}.",
    ),
];

/// The phrase of an operator, if it has one.
pub fn phrase(op: &str) -> Option<&'static str> {
    phrase_in(op, Lang::En)
}

/// The phrase of an operator in a language, if it has one.
pub fn phrase_in(op: &str, lang: Lang) -> Option<&'static str> {
    let table = if lang == Lang::Pt { PHRASES_PT } else { PHRASES };
    table.iter().find(|(o, _)| *o == op).map(|(_, p)| *p)
}

/// The operators with a phrase.
pub fn operators() -> impl Iterator<Item = &'static str> {
    PHRASES.iter().map(|(o, _)| *o)
}

/// A setting's value in words: given, else the operator's default (none: neither).
fn value(n: &Node, sig: Option<&OpSignature>, name: &str, given_only: bool, lang: Lang) -> Option<String> {
    // (an empty text is none given)
    if let Some(v) = n.params.get(name).filter(|v| v.as_str() != Some("")) {
        return Some(crate::document::words_in(v, lang).replace('\n', "; "));
    }
    if given_only {
        return None;
    }
    let doc = sig?.params.iter().find(|p| p.name == name)?.doc.clone();
    let d = crate::node_view::default_of(&doc);
    let d = d.strip_prefix("default ").unwrap_or(&d).trim().to_string();
    if !d.is_empty() && d != "default" {
        return Some(d);
    }
    // "8 (default) or 4": the word before it
    let before = doc.split_once(" (default)")?.0;
    let word = before.rsplit([' ', ':', ',', '(']).next()?.trim();
    (!word.is_empty()).then(|| word.to_string())
}

/// Where an input comes from, in a word: the node (not its port), the model's input, the value.
fn source(n: &Node, port: &str) -> Option<String> {
    Some(match From::of(n.inputs.get(port)?) {
        From::Node { node, .. } | From::Within { node, .. } => node,
        From::Input(name) | From::Carried(name) => name,
        From::Value(v) => v,
    })
}

/// Fill a template's placeholders (`given_only`: inside brackets, nothing taken from the defaults); none when one
/// has no value.
fn fill(t: &str, n: &Node, sig: Option<&OpSignature>, given_only: bool, lang: Lang) -> Option<String> {
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find(['{', '<']) {
        out.push_str(&rest[..at]);
        let close = if rest[at..].starts_with('{') { '}' } else { '>' };
        let end = at + rest[at..].find(close)?;
        let name = &rest[at + 1..end];
        let v = if close == '}' { value(n, sig, name, given_only, lang)? } else { source(n, name)? };
        out.push_str(&v);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// The sentence of a step: its operator's phrase with its settings and inputs (none: no phrase, or a setting it
/// needs has no value).
pub fn sentence(n: &Node, sig: Option<&OpSignature>) -> Option<String> {
    sentence_in(n, sig, Lang::En)
}

/// The sentence of a step in a language.
pub fn sentence_in(n: &Node, sig: Option<&OpSignature>, lang: Lang) -> Option<String> {
    let t = phrase_in(&n.op, lang)?;
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find('[') {
        out.push_str(&fill(&rest[..at], n, sig, false, lang)?);
        let end = at + rest[at..].find(']')?;
        out.push_str(&fill(&rest[at + 1..end], n, sig, true, lang).unwrap_or_default());
        rest = &rest[end + 1..];
    }
    out.push_str(&fill(rest, n, sig, false, lang)?);
    // (a sentence starts with a capital, an input's name first too)
    let mut c = out.chars();
    Some(c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Registry;

    /// The settings a step's sentence says (left out of the list under it, unless they differ from the default).
    fn said(op: &str) -> std::collections::BTreeMap<&'static str, ()> {
        let mut out = std::collections::BTreeMap::new();
        if let Some(t) = phrase(op) {
            let mut rest = t;
            while let Some(at) = rest.find('{') {
                let Some(end) = rest[at..].find('}') else { break };
                out.insert(&rest[at + 1..at + end], ());
                rest = &rest[at + end + 1..];
            }
        }
        out
    }

    #[test]
    fn the_templates_steps_read_as_sentences() {
        let r = Registry::standard();
        fn walk(nodes: &[Node], r: &Registry, out: &mut Vec<String>) {
            for n in nodes {
                let sig = r.sig(&n.op).map(|s| s.as_ref().clone());
                for lang in [Lang::En, Lang::Pt] {
                    if let Some(t) = sentence_in(n, sig.as_ref(), lang) {
                        out.push(format!("{} {}: {t}", n.op, lang.code()));
                    }
                }
                if let Some(b) = &n.body {
                    walk(&b.nodes, r, out);
                }
            }
        }
        let mut said = Vec::new();
        for t in crate::templates::TEMPLATES {
            walk(&t.model().nodes, &r, &mut said);
        }
        if std::env::var("TERRAFLUX_PRINT").is_ok() {
            said.sort();
            said.dedup();
            for s in &said {
                eprintln!("{s}");
            }
        }
        for s in &said {
            assert!(!s.contains(['{', '}']) || s.contains("rules") || s.contains("expression"), "{s}");
            assert!(!s.contains("[ ") && !s.contains(" ]"), "{s}");
        }
        assert!(said.len() > 200, "{}", said.len());
    }

    #[test]
    fn a_step_reads_as_a_sentence() {
        let r = Registry::standard();
        let sig = |op: &str| r.sig(op).map(|s| s.as_ref().clone());
        let n = Node::new("aptitude", "nlm.gaussian_field").param("rows", 100).param("columns", 120).param("seed", 7);
        assert_eq!(
            sentence(&n, sig("nlm.gaussian_field").as_ref()).as_deref(),
            Some(
                "A Gaussian random field of 100 × 120 cells: autocorrelation range 10 cells, variance 5, nugget 0.2; seed 7."
            )
        );
        let n = Node::new("habitat", "nlm.binarize").input("map", "aptitude.map").param("breaks", 0.3);
        assert_eq!(
            sentence(&n, sig("nlm.binarize").as_ref()).as_deref(),
            Some("Aptitude split into habitat and matrix, a share 0.3 of it habitat.")
        );
        let n = Node::new("chain", "markov.fit").input("t0", "$landuse_2000").input("t1", "$landuse_2010");
        assert_eq!(
            sentence(&n, sig("markov.fit").as_ref()).as_deref(),
            Some("The Markov chain of the changes from landuse_2000 to landuse_2010.")
        );
        // in Portuguese, the same operators, settings and inputs
        let marks = |t: &str| {
            let mut m: Vec<String> = t
                .split(['{', '<'])
                .skip(1)
                .filter_map(|p| p.split_once(['}', '>']).map(|(a, _)| a.to_string()))
                .collect();
            m.sort();
            m
        };
        for op in operators() {
            let pt = phrase_in(op, Lang::Pt).unwrap_or_else(|| panic!("{op}: no phrase in Portuguese"));
            assert_eq!(marks(pt), marks(phrase(op).unwrap()), "{op}");
            assert_eq!(pt.matches('[').count(), phrase(op).unwrap().matches('[').count(), "{op}");
        }
        assert_eq!(PHRASES_PT.len(), PHRASES.len());
        let n = Node::new("habitat", "nlm.binarize").input("map", "aptitude.map").param("breaks", 0.3);
        assert_eq!(
            sentence_in(&n, sig("nlm.binarize").as_ref(), Lang::Pt).as_deref(),
            Some("Aptitude dividido em habitat e matriz, uma fração 0.3 dele habitat.")
        );
        // an input it needs not given: no sentence
        assert_eq!(sentence(&Node::new("k", "validate.kappa"), sig("validate.kappa").as_ref()), None);
        // every phrase names settings and inputs its operator has
        for op in operators() {
            let s = sig(op).unwrap_or_else(|| panic!("{op}: no such operator"));
            for name in said(op).keys() {
                assert!(s.params.iter().any(|p| p.name == *name), "{op}: no setting {name}");
            }
            let mut rest = phrase(op).unwrap();
            while let Some(at) = rest.find('<') {
                let end = at + rest[at..].find('>').unwrap();
                let port = &rest[at + 1..end];
                assert!(s.inputs.iter().any(|p| p.name == port), "{op}: no input {port}");
                rest = &rest[end + 1..];
            }
        }
    }
}
