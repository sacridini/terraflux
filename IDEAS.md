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

## Vetores

Os vetores entraram em 0.13.0 (leitura e escrita pelo OGR, rasterização, estatísticas zonais, somas exatas,
distâncias, unidades de planejamento em polígonos, o desenho no viewer).
- **Feito** (Unreleased): vetores sobre os mapas no viewer, e sozinhos, desenhados em tiles por threads a partir
  de um R-tree (camadas de milhões de feições sem travar), coloridos por atributo; `vector.extract` (o do
  landschaft, bit a bit), `vector.buffer`, `vector.clip` e `vector.dissolve` (o GEOS pelo GDAL), `vector.join`.
- **Feito** (Unreleased): os vetores no motor por tiles: `vector.zonal` por faixas de linhas (as zonas grandes
  corrida por corrida, as mesmas somas do C++), `vector.extract` pelas faixas que têm pontos e `vector.rasterize`
  num GeoTIFF faixa a faixa, bit a bit com a memória (exceto as linhas do GDAL com `all_touched` nos cantos das
  células). Falta: `vector.exact_sum` e `vector.distance` fora da memória.

## Viewer

- O viewer escolhe as cores pelo tipo do dado (classes, rampa, divergente em torno de 0, terreno, hillshade,
  NDVI, MSPA), pelo operador que fez o valor e pelos nomes de nós, arquivos e bandas.
- **Feito** (Unreleased): o produto gravado com o mapa (`landschaft_product` no GeoTIFF) e lido de volta; a
  escala logarítmica (correntes de circuito, e à mão); a rampa pelos percentis 2–98; a paleta trocada à mão no
  painel e lembrada por saída.

## Builder

- **Feito** (Unreleased): navegar pelos nós com o teclado (setas com o ponteiro sobre o canvas, Enter abre a
  saída); templates do Dinamica EGO e do CLUE-S (dados desenhados por modelos neutros); templates do próprio
  usuário (salvar o modelo como template, vê-lo e removê-lo no menu).
