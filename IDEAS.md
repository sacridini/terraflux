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
   - FAD e FAD multiescala (`fad.cpp`), FOS (`fos.cpp`): densidade e fragmentação da floresta.
   - SPA (`spa.cpp`): o MSPA simplificado.
   - Accounting (`accounting.cpp`): manchas por tamanho (dá um mapa e uma tabela).
   - Depois: contagion/PFF e clustering (`contagion.cpp`), entropia espacial (`entropy.cpp`), parcellation e
     contortion (`morphology.cpp`), GTN (`gtn.cpp`), restoration, influence, isochrone, gray spatial
     convolution, mudança (`change.cpp`, `fad_change`).
   - Cada produto com a sua paleta e os nomes das classes no viewer, como o MSPA (os do landschaft estão em
     `src/landschaft/_plotting/_palettes.py`: classes do FAD, códigos de densidade, mudança simples e
     morfológica, a ordem da legenda do GTB).
2. **Métricas de paisagem** (`lsm`, no estilo do FRAGSTATS/landscapemetrics, em
   `extern/landschaft/src/cpp/lsm`). Devolvem tabelas por mancha, classe e paisagem, e janelas móveis devolvem
   mapas. Antes de valer a pena, o app precisa de um visualizador de tabelas melhor (ordenar, filtrar, exportar)
   e de gráficos das métricas por classe.

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
- Mais templates: um modelo do Dinamica EGO com os dados de exemplo, um CLUE-S, FAD/FOS quando entrarem.
- Templates do próprio usuário: salvar um modelo como template e vê-lo no menu.
