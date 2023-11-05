use crate::range::Range;
use crate::SgRoot;

use ast_grep_config::{GlobalRules, SerializableRuleCore};
use ast_grep_core::{NodeMatch, StrDoc};
use ast_grep_language::SupportLang;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Context;
use once_cell::sync::Lazy;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;

static GLOBAL_RULES: Lazy<GlobalRules<SupportLang>> = Lazy::new(GlobalRules::default);

#[pyclass(mapping)]
pub struct SgNode {
  pub inner: NodeMatch<'static, StrDoc<SupportLang>>,
  // refcount SgRoot
  pub(crate) root: Py<SgRoot>,
}

// it is safe to send tree-sitter Node
// because it is refcnt and concurrency safe
unsafe impl Send for SgNode {}

#[pymethods]
impl SgNode {
  /*----------  Node Inspection ----------*/
  fn range(&self) -> Range {
    Range::from(&self.inner)
  }

  fn is_leaf(&self) -> bool {
    self.inner.is_leaf()
  }

  fn is_named(&self) -> bool {
    self.inner.is_named()
  }

  fn is_named_leaf(&self) -> bool {
    self.inner.is_named_leaf()
  }

  fn kind(&self) -> String {
    self.inner.kind().to_string()
  }

  fn text(&self) -> String {
    self.inner.text().to_string()
  }

  /*---------- Search Refinement  ----------*/
  #[pyo3(signature = (**kwargs))]
  fn matches(&self, kwargs: Option<&PyDict>) -> PyResult<bool> {
    let config = config_from_rule(self.inner.lang(), kwargs.unwrap());
    let matcher = config
      .get_matcher(&GLOBAL_RULES)
      .context("cannot get matcher")?;
    Ok(self.inner.matches(matcher))
  }

  #[pyo3(signature = (**kwargs))]
  fn inside(&self, kwargs: Option<&PyDict>) -> bool {
    let config = config_from_rule(self.inner.lang(), kwargs.unwrap());
    let matcher = config.get_matcher(&GLOBAL_RULES).unwrap();
    self.inner.inside(matcher)
  }

  #[pyo3(signature = (**kwargs))]
  fn has(&self, kwargs: Option<&PyDict>) -> bool {
    let config = config_from_rule(self.inner.lang(), kwargs.unwrap());
    let matcher = config.get_matcher(&GLOBAL_RULES).unwrap();
    self.inner.has(matcher)
  }

  #[pyo3(signature = (**kwargs))]
  fn precedes(&self, kwargs: Option<&PyDict>) -> bool {
    let config = config_from_rule(self.inner.lang(), kwargs.unwrap());
    let matcher = config.get_matcher(&GLOBAL_RULES).unwrap();
    self.inner.precedes(matcher)
  }

  #[pyo3(signature = (**kwargs))]
  fn follows(&self, kwargs: Option<&PyDict>) -> bool {
    let config = config_from_rule(self.inner.lang(), kwargs.unwrap());
    let matcher = config.get_matcher(&GLOBAL_RULES).unwrap();
    self.inner.follows(matcher)
  }

  fn get_match(&self, meta_var: &str) -> Option<Self> {
    self
      .inner
      .get_env()
      .get_match(meta_var)
      .cloned()
      .map(|n| Self {
        inner: NodeMatch::from(n),
        root: self.root.clone(),
      })
  }

  fn get_multiple_matches(&self, meta_var: &str) -> Vec<SgNode> {
    self
      .inner
      .get_env()
      .get_multiple_matches(meta_var)
      .into_iter()
      .map(|n| Self {
        inner: NodeMatch::from(n),
        root: self.root.clone(),
      })
      .collect()
  }

  /*---------- Tree Traversal  ----------*/
  fn get_root(&self) -> Py<SgRoot> {
    self.root.clone()
  }

  #[pyo3(signature = (config=None, **kwargs))]
  fn find(&self, config: Option<&PyDict>, kwargs: Option<&PyDict>) -> Option<Self> {
    let config = self.get_config(config, kwargs);
    let matcher = config.get_matcher(&GLOBAL_RULES).unwrap();
    let inner = self.inner.find(matcher)?;
    Some(Self {
      inner,
      root: self.root.clone(),
    })
  }
  #[pyo3(signature = (config=None, **kwargs))]
  fn find_all(&self, config: Option<&PyDict>, kwargs: Option<&PyDict>) -> Vec<Self> {
    let config = self.get_config(config, kwargs);
    let matcher = config.get_matcher(&GLOBAL_RULES).unwrap();
    self
      .inner
      .find_all(matcher)
      .map(|n| Self {
        inner: n,
        root: self.root.clone(),
      })
      .collect()
  }

  fn field(&self, name: &str) -> Option<SgNode> {
    self.inner.field(name).map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn parent(&self) -> Option<SgNode> {
    self.inner.parent().map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn child(&self, nth: usize) -> Option<SgNode> {
    self.inner.child(nth).map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn ancestors(&self) -> Vec<SgNode> {
    self
      .inner
      .ancestors()
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
  }

  fn children(&self) -> Vec<SgNode> {
    self
      .inner
      .children()
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
  }

  fn next(&self) -> Option<SgNode> {
    self.inner.next().map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn next_all(&self) -> Vec<SgNode> {
    self
      .inner
      .next_all()
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
  }

  fn prev(&self) -> Option<SgNode> {
    self.inner.prev().map(|inner| Self {
      inner: inner.into(),
      root: self.root.clone(),
    })
  }

  fn prev_all(&self) -> Vec<SgNode> {
    self
      .inner
      .prev_all()
      .map(|inner| Self {
        inner: inner.into(),
        root: self.root.clone(),
      })
      .collect()
  }

  /*---------- Magic Method  ----------*/
  fn __hash__(&self) -> u64 {
    let mut s = DefaultHasher::new();
    self.inner.node_id().hash(&mut s);
    s.finish()
  }
  fn __eq__(&self, other: &Self) -> bool {
    self.inner.node_id() == other.inner.node_id()
  }
  fn __str__(&self) -> String {
    let range = self.range();
    format!("{}@{}", self.inner.kind(), range)
  }
  fn __repr__(&self) -> String {
    let range = self.range();
    let chars: Vec<_> = self.text().chars().take(10).collect();
    let src = if chars.len() > 9 {
      let s: String = chars.into_iter().take(5).collect();
      format!("{}...", s)
    } else {
      chars.into_iter().collect()
    };
    format!("SgNode(`{src}`, kind={}, range={range})", self.inner.kind())
  }
  fn __getitem__(&self, key: &str) -> PyResult<Self> {
    if let Some(node) = self.get_match(key) {
      Ok(node)
    } else {
      Err(PyErr::new::<PyKeyError, _>(key.to_string()))
    }
  }
}

impl SgNode {
  fn get_config(
    &self,
    config: Option<&PyDict>,
    kwargs: Option<&PyDict>,
  ) -> SerializableRuleCore<SupportLang> {
    let lang = self.inner.lang();
    if let Some(config) = config {
      config_from_dict(lang, config)
    } else {
      // TODO: remove unwrap
      config_from_rule(lang, kwargs.unwrap())
    }
  }
}

fn config_from_dict(lang: &SupportLang, dict: &PyDict) -> SerializableRuleCore<SupportLang> {
  dict.set_item("language", lang.to_string()).unwrap();
  depythonize(dict).unwrap()
}

fn config_from_rule(lang: &SupportLang, dict: &PyDict) -> SerializableRuleCore<SupportLang> {
  let rule = depythonize(dict).unwrap();
  SerializableRuleCore {
    language: *lang,
    rule,
    constraints: None,
    utils: None,
    transform: None,
  }
}
