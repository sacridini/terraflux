//! A fitted species distribution model (MaxNet, a GLM or a random forest) and what fits one, as landschaft's fit
//! takes any estimator: its suitability (the presence's probability, or MaxNet's output) for rows of predictors.

use serde::{Deserialize, Serialize};

use super::extrapolation::Frame;
use super::forest::{self, Forest};
use super::logistic::{self, Logistic};
use super::maxnet::{self, MaxNet};

/// A fitted model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Model {
    Maxnet(Box<MaxNet>),
    Glm(Logistic),
    RandomForest(Forest),
}

impl Model {
    /// Its suitability for every row: MaxNet's output (cloglog by default), the others' probability of presence.
    pub fn suitability(&self, x: &Frame) -> Result<Vec<f64>, String> {
        match self {
            Model::Maxnet(m) => m.predict(x, None, None),
            Model::Glm(m) => m.predict(x),
            Model::RandomForest(m) => m.predict(x),
        }
    }

    /// The predictors it takes, in its order.
    pub fn predictors(&self) -> Vec<String> {
        match self {
            Model::Maxnet(m) => m.predictors(),
            Model::Glm(m) => m.predictors.clone(),
            Model::RandomForest(m) => m.predictors.clone(),
        }
    }

    /// Its kind: maxnet, glm or random_forest.
    pub fn kind(&self) -> &'static str {
        match self {
            Model::Maxnet(_) => "maxnet",
            Model::Glm(_) => "glm",
            Model::RandomForest(_) => "random_forest",
        }
    }
}

/// What fits a model: MaxNet's, the GLM's or the forest's settings.
pub enum Learner<'a> {
    Maxnet(maxnet::Settings<'a>),
    Glm(logistic::Settings),
    RandomForest(forest::Settings),
}

impl Learner<'_> {
    /// The model fitted to the records, with their weights (MaxNet takes none, as landschaft's).
    pub fn fit(&self, x: &Frame, y: &[f64], weights: Option<&[f64]>) -> Result<Model, String> {
        Ok(match self {
            Learner::Maxnet(s) => Model::Maxnet(Box::new(maxnet::fit(x, y, s)?)),
            Learner::Glm(s) => Model::Glm(logistic::fit(x, y, weights, s)?),
            Learner::RandomForest(s) => Model::RandomForest(forest::fit(x, y, weights, s)?),
        })
    }
}
