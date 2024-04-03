use promql_parser::parser::{Expr, VectorSelector};
use promql_parser::util::{walk_expr, ExprVisitor};

use crate::metrics::types::InstantQuery;
use crate::metrics::types::RangeQuery;
use actix_web::web::Query;
use actix_web::HttpResponse;
use log::{error, info, warn};
use promql_parser::label::MatchOp;
use promql_parser::parser;

// https://prometheus.io/docs/prometheus/latest/querying/api/
pub struct NamespaceVisitor {
    pub namespace: String,
}

pub trait PromQuery {
    fn get_query(&self) -> &str;
}

impl PromQuery for RangeQuery {
    fn get_query(&self) -> &str {
        &self.query
    }
}

impl PromQuery for InstantQuery {
    fn get_query(&self) -> &str {
        &self.query
    }
}

// Vector selector is the part in prometheus query that selects the metrics
// Example: (sum by (namespace) (container_memory_usage_bytes))
// container_memory_usage_bytes is the vector selector.
// We require all vector selectors to have a label namespace
// For example like this (sum by (namespace) (container_memory_usage_bytes{namespace="org-foo-inst-bar"}))
fn validate_vector_selector(namespace: &String, vector_selector: &VectorSelector) -> bool {
    let mut authorized_query = false;
    for filters in &vector_selector.matchers.matchers {
        if filters.name == "namespace"
            && filters.value == *namespace
            && filters.op == MatchOp::Equal
        {
            authorized_query = true;
        }
    }
    authorized_query
}

// This checks that prometheus queries are only using authorized namespace
impl ExprVisitor for NamespaceVisitor {
    type Error = &'static str; // Using a simple error type for this example.

    fn pre_visit(&mut self, expr: &Expr) -> Result<bool, Self::Error> {
        match expr {
            Expr::VectorSelector(vector_selector) => {
                let authorized_query = validate_vector_selector(&self.namespace, vector_selector);
                if !authorized_query {
                    return Ok(false);
                }
            }
            Expr::MatrixSelector(matrix_selector) => {
                let authorized_query =
                    validate_vector_selector(&self.namespace, &matrix_selector.vs);
                if !authorized_query {
                    return Ok(false);
                }
            }
            Expr::Call(call) => {
                for boxed_arg in &call.args.args {
                    let expr_arg = boxed_arg;
                    match self.pre_visit(expr_arg) {
                        Ok(true) => (),
                        Ok(false) => return Ok(false),
                        Err(e) => return Err(e),
                    }
                }
            }
            Expr::Extension(_) => {
                return Err("Using PromQL extensions is not allowed");
            }
            _ => (),
        }
        // Continue to the rest of the tree.
        Ok(true)
    }
}

// Returns the query if it's valid
// otherwise returns an error in the form of HttpResponse
pub fn check_query_only_accesses_namespace<T: PromQuery>(
    query: &Query<T>,
    namespace: &String,
) -> Result<String, HttpResponse> {
    // Get the query parameters
    let query_str = query.get_query();

    // Parse the query
    let abstract_syntax_tree = match parser::parse(query_str) {
        Ok(ast) => ast,
        Err(e) => {
            error!("Query parse error: {}", e);
            return Err(HttpResponse::UnprocessableEntity().json("Failed to parse PromQL query"));
        }
    };

    // Recurse through all terms in the expression to find any terms that specify
    // label matching, and make sure all of them specify the namespace label.
    let mut visitor = NamespaceVisitor {
        namespace: namespace.clone(),
    };
    let all_metrics_specify_namespace = walk_expr(&mut visitor, &abstract_syntax_tree);

    // Check if we are performing an unauthorized query.
    match all_metrics_specify_namespace {
        Ok(true) => {
            info!(
                "Authorized request: namespace '{}', query '{}'",
                namespace, query_str
            );
        }
        _ => {
            warn!(
                "Unauthorized request: namespace '{}', query '{}'",
                namespace, query_str
            );
            return Err(
                HttpResponse::Forbidden().json("Must include namespace in all vector selectors")
            );
        }
    }
    Ok(query_str.to_string())
}
