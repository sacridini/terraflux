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
   - a análise espaço-temporal e a de assinaturas (PCA, NMF, agrupamentos: `lsm/analysis.py`, em Python);
   - amostragem por pontos e buffers (`sample_lsm`, `lsm/sampling.py`) e métricas por zonas (`grid_zones`);
   - paisagens fora da memória (`TiledLandscape`, já em C++), para rasters maiores que a RAM;
   - a correlação entre métricas (`r_cor`) e gráficos de várias métricas de uma vez.
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
   - **Feito** (0.13.0): unidades de planejamento em polígonos no prioritizr (com os vetores no builder). Falta:
     polígonos com zonas de manejo (o `polygon_zone_units` do landschaft) e as unidades H3 (`h3_units`).

## Vetores

Os vetores entraram em 0.13.0 (leitura e escrita pelo OGR, rasterização, estatísticas zonais, somas exatas,
distâncias, unidades de planejamento em polígonos, o desenho no viewer). Falta:
- desenhar vetores sobre um mapa no viewer (hoje cada um no seu painel);
- extrair valores de mapas em pontos (o `extract` do landschaft, bilinear ou da célula);
- buffers, recortes e uniões de geometrias (o GEOS pelo GDAL);
- os vetores no motor por tiles.

## Viewer

- O viewer escolhe as cores pelo tipo do dado (classes, rampa, divergente em torno de 0, terreno, hillshade,
  NDVI, MSPA), pelo operador que fez o valor e pelos nomes de nós, arquivos e bandas. Falta:
  - o produto viajar com o valor (um atributo como o `landschaft_product`, gravado também no GeoTIFF), em vez de
    adivinhar pelo nome do arquivo;
  - escala logarítmica para correntes de circuito e contagens muito assimétricas (o `choose_cmap` do
    landschaft);
  - a faixa da rampa pelos percentis 2–98 em vez de mínimo e máximo;
  - trocar a paleta à mão no painel do mapa, e lembrar a escolha por nó.

## Builder

- Navegar pelos nós com o teclado (setas para o nó vizinho, Enter para abrir o mapa), completando o "/" para
  montar um modelo inteiro sem o mouse.
- Mais templates: um modelo do Dinamica EGO com os dados de exemplo, um CLUE-S. **Feito** (Unreleased): FAD/FOS,
  SPA e Accounting.
- Templates do próprio usuário: salvar um modelo como template e vê-lo no menu.
