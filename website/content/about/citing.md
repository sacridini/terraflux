---
title: Cite Terraflux
description: How to cite Terraflux and, above all, the papers of the methods you used through it.
order: 10
---

When a study uses Terraflux, cite **two things**: Terraflux itself, the software that ran the analysis, and the
**papers of the methods** you used through it. Terraflux re-implements methods developed by other researchers;
their papers are what a reader needs in order to understand, judge and repeat your analysis, and citing them gives
credit to the people who created the methods.

## Terraflux

Cite the version you used (`terraflux --version` prints it):

> Lacerda, E. (2026). *Terraflux: landscape ecology and land change modelling from a laptop to a cluster*
> (Version {{version}}) [Computer software]. https://github.com/sacridini/terraflux

```bibtex
@software{terraflux,
  author  = {Lacerda, Eduardo},
  title   = {Terraflux: landscape ecology and land change modelling from a laptop to a cluster},
  version = {{{version}}},
  year    = {2026},
  url     = {https://github.com/sacridini/terraflux},
  license = {GPL-3.0-or-later}
}
```

A paper describing Terraflux is in preparation; this page will give its reference when it is published.

## The methods

Each method below gives the same results as the reference implementation named, which Terraflux's tests check
against. In a methods section, a sentence such as *"MSPA (Soille & Vogt, 2009) was computed with Terraflux
{{version}}, which reproduces GuidosToolbox (Vogt & Riitters, 2017)"* tells the reader both what was done and how.

| Method | Operators | Cite |
|---|---|---|
| Morphological spatial pattern analysis (MSPA), fragmentation (FAD, FOS), accounting | `landscape.*`, `fragmentation.*` | Soille & Vogt (2009); Vogt & Riitters (2017) |
| Landscape metrics (patch, class, landscape, moving windows) | `metrics.*` | McGarigal & Marks (1995); Hesselbarth et al. (2019) |
| Habitat availability and connectivity indices (PC, IIC, dPC) | `conn.*`, `networks.*` | Saura & Pascual-Hortal (2007); Saura & Torné (2009) |
| Circuit theory (Circuitscape) and Omniscape | `circuit.*` | McRae et al. (2008); Landau et al. (2021) |
| Least-cost and random-walk distances (gdistance) | `cost.*` | van Etten (2017) |
| Zonation 5 | `prioritize.zonation*` | Moilanen et al. (2022) |
| Prioritization by integer linear programming | `prioritize.optimize` and related | Hanson et al. (2025) |
| Multi-action planning | `prioritize.actions*` | Salgado-Rojas et al. (2020) |
| Restoration planning | `restoration.*` | Strassburg et al. (2020) |
| Maxent (MaxNet) species distribution models | `sdm.*` | Phillips et al. (2017) |
| Model tuning and evaluation (ENMeval) | `sdm.*` | Kass et al. (2021) |
| Cleaning occurrence records (CoordinateCleaner) | `sdm.clean_occurrences` | Zizka et al. (2019) |
| Biodiverse's analyses | `bio.*` | Laffan et al. (2010) |
| Dinamica EGO's model (weights of evidence, Expander, Patcher) | `woe.*`, `patches.*`, `dinamica.*` | Soares-Filho et al. (2002) |
| CLUE-S and the lulcc framework | `glm.*`, `lulcc.*`, `demand.*` | Verburg et al. (2002); Moulds et al. (2015) |
| PLUS (LEAS and CARS) | `plus.*`, `cars.*` | Liang et al. (2021) |
| FUTURES | `futures.*` | Meentemeyer et al. (2013) |
| CRAFTY | `crafty.allocate` | Murray-Rust et al. (2014) |

Each tutorial lists, at its end, the papers behind the method it teaches, including those of variants and
settings not in this table. Cite those you used.

In the application, **Methods and citations** (File menu, or the Document pane) writes this for your model: a
paragraph naming each method it uses with its papers and the software reproduced, the model step by step, and
the references of this table that apply, to copy or save as BibTeX or RIS (see
[The model's document](/tutorials/building-models/the-model-document/#methods-and-citations)).

## Make the analysis reproducible

A citation says which software ran the analysis; the files let a reader run it again. In a paper's supplementary
material (or a data repository), put:

- **the model files** (`.toml`), with the submodels they use: they hold every step, setting and seed;
- **the `manifest.json` of each run you report**, written by `terraflux run` (and by a whole run of a saved model
  in the application) beside its outputs: it records the version of Terraflux, the SHA-256 fingerprints of the
  model and of its input files, the values and seeds used, and the fingerprint of every output, so a reader can
  tell that a file they have is the one you reported. `terraflux reproduce manifest.json` runs it again and says
  whether every output is the same, byte for byte;
- **the data**, or where to get them, when they can be shared;
- the model's **tests** (`[[tests]]` and their `expected` files) when it has them: `terraflux test` checks the
  results on another computer, within each test's tolerance (the manifest names full paths, so `reproduce` works
  where the files are at the same place).

[Checks, tests and reproducible runs](/tutorials/building-models/checks-and-tests/) shows each of these on an
example; [Randomness and reproducibility](/tutorials/building-models/randomness/) explains why the same seed gives
the same bits.

## References

- Hanson, J. O., Schuster, R., Strimas-Mackey, M., Morrell, N., Edwards, B. P. M., Arcese, P., Bennett, J. R., & Possingham, H. P. (2025). Systematic conservation prioritization with the prioritizr R package. *Conservation Biology*, 39(1), e14376. https://doi.org/10.1111/cobi.14376
- Hesselbarth, M. H. K., Sciaini, M., With, K. A., Wiegand, K., & Nowosad, J. (2019). landscapemetrics: an open-source R tool to calculate landscape metrics. *Ecography*, 42(10), 1648–1657. https://doi.org/10.1111/ecog.04617
- Kass, J. M., Muscarella, R., Galante, P. J., Bohl, C. L., Pinilla-Buitrago, G. E., Boria, R. A., Soley-Guardia, M., & Anderson, R. P. (2021). ENMeval 2.0: Redesigned for customizable and reproducible modeling of species' niches and distributions. *Methods in Ecology and Evolution*, 12(9), 1602–1608. https://doi.org/10.1111/2041-210X.13628
- Laffan, S. W., Lubarsky, E., & Rosauer, D. F. (2010). Biodiverse, a tool for the spatial analysis of biological and related diversity. *Ecography*, 33(4), 643–647. https://doi.org/10.1111/j.1600-0587.2010.06237.x
- Landau, V. A., Shah, V. B., Anantharaman, R., & Hall, K. R. (2021). Omniscape.jl: Software to compute omnidirectional landscape connectivity. *Journal of Open Source Software*, 6(57), 2829. https://doi.org/10.21105/joss.02829
- Liang, X., Guan, Q., Clarke, K. C., Liu, S., Wang, B., & Yao, Y. (2021). Understanding the drivers of sustainable land expansion using a patch-generating land use simulation (PLUS) model: A case study in Wuhan, China. *Computers, Environment and Urban Systems*, 85, 101569. https://doi.org/10.1016/j.compenvurbsys.2020.101569
- McGarigal, K., & Marks, B. J. (1995). *FRAGSTATS: spatial pattern analysis program for quantifying landscape structure*. General Technical Report PNW-GTR-351. USDA Forest Service, Pacific Northwest Research Station.
- McRae, B. H., Dickson, B. G., Keitt, T. H., & Shah, V. B. (2008). Using circuit theory to model connectivity in ecology, evolution, and conservation. *Ecology*, 89(10), 2712–2724. https://doi.org/10.1890/07-1861.1
- Meentemeyer, R. K., Tang, W., Dorning, M. A., Vogler, J. B., Cunniffe, N. J., & Shoemaker, D. A. (2013). FUTURES: Multilevel simulations of emerging urban–rural landscape structure using a stochastic patch-growing algorithm. *Annals of the Association of American Geographers*, 103(4), 785–807. https://doi.org/10.1080/00045608.2012.707591
- Moilanen, A., Lehtinen, P., Kohonen, I., Jalkanen, J., Virtanen, E. A., & Kujala, H. (2022). Novel methods for spatial prioritization with applications in conservation, land use planning and ecological impact avoidance. *Methods in Ecology and Evolution*, 13(5), 1062–1072. https://doi.org/10.1111/2041-210X.13819
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Murray-Rust, D., Brown, C., van Vliet, J., Alam, S. J., Robinson, D. T., Verburg, P. H., & Rounsevell, M. (2014). Combining agent functional types, capitals and services to model land use dynamics. *Environmental Modelling & Software*, 59, 187–201. https://doi.org/10.1016/j.envsoft.2014.05.019
- Phillips, S. J., Anderson, R. P., Dudík, M., Schapire, R. E., & Blair, M. E. (2017). Opening the black box: an open-source release of Maxent. *Ecography*, 40(7), 887–893. https://doi.org/10.1111/ecog.03049
- Salgado-Rojas, J., Álvarez-Miranda, E., Hermoso, V., Garcia-Gonzalo, J., & Weintraub, A. (2020). A mixed integer programming approach for multi-action planning for threat management. *Ecological Modelling*, 418, 108901. https://doi.org/10.1016/j.ecolmodel.2019.108901
- Saura, S., & Pascual-Hortal, L. (2007). A new habitat availability index to integrate connectivity in landscape conservation planning: Comparison with existing indices and application to a case study. *Landscape and Urban Planning*, 83(2–3), 91–103. https://doi.org/10.1016/j.landurbplan.2007.03.005
- Saura, S., & Torné, J. (2009). Conefor Sensinode 2.2: A software package for quantifying the importance of habitat patches for landscape connectivity. *Environmental Modelling & Software*, 24(1), 135–139. https://doi.org/10.1016/j.envsoft.2008.05.005
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
- Strassburg, B. B. N., Iribarrem, A., Beyer, H. L., Cordeiro, C. L., Crouzeilles, R., Jakovac, C. C., et al. (2020). Global priority areas for ecosystem restoration. *Nature*, 586(7831), 724–729. https://doi.org/10.1038/s41586-020-2784-9
- van Etten, J. (2017). R package gdistance: Distances and routes on geographical grids. *Journal of Statistical Software*, 76(13), 1–21. https://doi.org/10.18637/jss.v076.i13
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Zizka, A., Silvestro, D., Andermann, T., Azevedo, J., Duarte Ritter, C., Edler, D., et al. (2019). CoordinateCleaner: Standardized cleaning of occurrence records from biological collection databases. *Methods in Ecology and Evolution*, 10(5), 744–751. https://doi.org/10.1111/2041-210X.13152
