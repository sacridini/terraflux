//! The Portuguese of what Terraflux writes for a reader in English: the report, the document's structure, the
//! checks' verdicts, a comparison's words. A text is looked up by its English, which is also what is written when
//! there is no translation; `{}` are the values put in it, in order.

use std::fmt::Display;

use crate::methods::Lang;

/// The English and its Portuguese.
const PT: &[(&str, &str)] = &[
    // the document
    ("Data", "Dados"),
    ("Preparation", "Preparação"),
    ("Model", "Modelo"),
    ("Validation", "Validação"),
    ("Results", "Resultados"),
    ("Analysis", "Análise"),
    ("Untitled", "Sem título"),
    ("a map", "um mapa"),
    ("a stack", "uma pilha"),
    ("a value", "um valor"),
    ("{}, band {}", "{}, banda {}"),
    ("{} to {}", "{} a {}"),
    (", by {}", ", passo {}"),
    ("at least {}", "no mínimo {}"),
    ("at most {}", "no máximo {}"),
    ("default", "padrão"),
    ("yes", "sim"),
    ("no", "não"),
    ("none", "nenhum"),
    // the report
    ("Request", "Pedido"),
    ("Author", "Autor"),
    ("Summary", "Resumo"),
    ("Checks", "Verificações"),
    ("On the method", "Sobre o método"),
    ("warning", "alerta"),
    ("advice", "conselho"),
    ("Inputs", "Entradas"),
    ("Input", "Entrada"),
    ("Kind", "Tipo"),
    ("Value", "Valor"),
    ("Note", "Nota"),
    ("Parameters", "Parâmetros"),
    ("Parameter", "Parâmetro"),
    ("Range", "Intervalo"),
    ("What it is", "O que é"),
    ("Scenarios", "Cenários"),
    ("Scenario", "Cenário"),
    ("Values", "Valores"),
    ("Outputs", "Saídas"),
    ("Output", "Saída"),
    ("From", "De"),
    ("File", "Arquivo"),
    ("Step", "Etapa"),
    ("Methods and citations", "Métodos e citações"),
    ("References", "Referências"),
    ("Assumptions", "Premissas"),
    ("Alternatives", "Alternativas"),
    ("Given in the model's notes", "Dadas nas notas do modelo"),
    ("No note.", "Sem nota."),
    ("Why.", "Por quê."),
    ("Not run.", "Não rodou."),
    (" of ", " de "),
    ("where {}", "onde {}"),
    ("Takes {}.", "Recebe {}."),
    ("Takes ", "Recebe "),
    ("where ", "onde "),
    ("(default {})", "(padrão {})"),
    ("Figure {}.", "Figura {}."),
    ("from {}", "de {}"),
    (", compared with {}", ", comparado com {}"),
    ("Repeated {} times.", "Repetido {} vezes."),
    ("Repeated ", "Repetido "),
    (" times.", " vezes."),
    ("Carries {}.", "Carrega {}."),
    ("Carries ", "Carrega "),
    (" from {}", " de {}"),
    (" from ", " de "),
    (", then ", ", depois "),
    ("Collects {}.", "Coleta {}."),
    ("Collects ", "Coleta "),
    ("Gives {}.", "Dá {}."),
    ("Gives ", "Dá "),
    ("Stops when {} is true.", "Para quando {} for verdadeiro."),
    ("Stops when ", "Para quando "),
    (" is true.", " for verdadeiro."),
    ("from ", "de "),
    (
        "Made by Terraflux {} on {} from the model's graph: every step is a node, in the order of what it depends on.",
        "Feito pelo Terraflux {} em {} a partir do grafo do modelo: cada etapa é um nó, na ordem do que ela depende.",
    ),
    (
        "Made by Terraflux {} on {} from the model's graph.",
        "Feito pelo Terraflux {} em {} a partir do grafo do modelo.",
    ),
    ("and {} more rows", "e mais {} linhas"),
    ("layer 1 of {}: {}", "camada 1 de {}: {}"),
    ("layer {} of {}", "camada {} de {}"),
    ("step {} of {}, {}", "passo {} de {}, {}"),
    ("step {} of {}", "passo {} de {}"),
    ("{} of {} columns", "{} de {} colunas"),
    ("the {} steps, {} of them animated", "os {} passos, {} deles animados"),
    ("the {} steps, animated", "os {} passos, animados"),
    // the checks
    ("the worst of the checks inside: {}", "a pior das verificações dentro: {}"),
    ("it held in the run", "valeu na execução"),
    ("the run did not come to it", "a execução não chegou a ela"),
    ("turned off (bypassed)", "desligada (contornada)"),
    ("passed", "passou"),
    ("warned", "avisou"),
    ("failed", "falhou"),
    ("not run", "não rodou"),
    ("off", "desligada"),
    ("failed, warning only: {}", "falhou, só um aviso: {}"),
    ("failed, the run stopped: {}", "falhou, a execução parou: {}"),
    ("not run yet", "ainda não rodou"),
    ("turned off", "desligada"),
    ("{} check: {}", "{} verificação: {}"),
    ("{} checks: {}", "{} verificações: {}"),
    // a comparison
    ("difference", "diferença"),
    ("{}: {} cells", "{}: {} células"),
    ("lower, to {}", "menor, até {}"),
    ("the same", "igual"),
    ("higher, to {}", "maior, até {}"),
    ("unchanged", "sem mudança"),
    ("other changes ({})", "outras mudanças ({})"),
    (" (every {} cells)", " (a cada {} células)"),
    ("no cell has a value in both maps", "nenhuma célula tem valor nos dois mapas"),
    ("no cell has a class in both maps", "nenhuma célula tem classe nos dois mapas"),
    (
        "the map compared minus this one: on average {}, from {} to {}; higher in {}% of the cells, lower in {}%",
        "o mapa comparado menos este: em média {}, de {} a {}; maior em {}% das células, menor em {}%",
    ),
    (
        "{} of {} cells changed class ({}%), in {} transition",
        "{} de {} células mudaram de classe ({}%), em {} transição",
    ),
    (
        "{} of {} cells changed class ({}%), in {} transitions",
        "{} de {} células mudaram de classe ({}%), em {} transições",
    ),
    (
        "The map compared is rotated, or of a coordinate system that cannot be carried to this map's: put it on \
         this map's grid (raster.match) to see the difference.",
        "O mapa comparado está rotacionado, ou em um sistema de coordenadas que não pode ser levado ao deste \
         mapa: ponha-o na grade deste mapa (raster.match) para ver a diferença.",
    ),
];

impl Lang {
    /// Its code, as a web page's `lang` says it.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Pt => "pt",
        }
    }

    /// A text in this language: its English when there is no translation.
    pub fn t(self, en: &'static str) -> &'static str {
        match self {
            Lang::En => en,
            Lang::Pt => PT.iter().find(|(e, _)| *e == en).map_or(en, |(_, p)| p),
        }
    }

    /// A text with values, `{}` each in order.
    pub fn f(self, en: &'static str, args: &[&dyn Display]) -> String {
        let t = self.t(en);
        let mut out = String::with_capacity(t.len() + 16 * args.len());
        let mut parts = t.split("{}");
        out.push_str(parts.next().unwrap_or_default());
        for (k, p) in parts.enumerate() {
            if let Some(a) = args.get(k) {
                out.push_str(&a.to_string());
            }
            out.push_str(p);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_text_has_one_translation_with_its_values() {
        for (k, (en, pt)) in PT.iter().enumerate() {
            assert!(PT[..k].iter().all(|(e, _)| e != en), "{en}: twice");
            assert_eq!(en.matches("{}").count(), pt.matches("{}").count(), "{en}");
        }
        assert_eq!(Lang::Pt.t("Inputs"), "Entradas");
        assert_eq!(Lang::En.t("Inputs"), "Inputs");
        assert_eq!(Lang::Pt.t("not translated"), "not translated");
        assert_eq!(Lang::Pt.f("Figure {}.", &[&3]), "Figura 3.");
        assert_eq!(Lang::En.f("layer {} of {}", &[&2, &5]), "layer 2 of 5");
        assert_eq!(Lang::Pt.code(), "pt");
    }
}
