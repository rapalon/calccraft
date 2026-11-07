//! The graphing mode's math: equations, their plots and their analysis. No UI code.
//!
//! Windows graphs with a proprietary engine (`GraphingImpl` is only a mock in the open
//! source code), so this module is CalcCraft's own, built to the same specification:
//!
//! - `parse` reads the equations the keypad and the keyboard write, with Windows'
//!   syntax errors, and `Equation::parse` sorts them into the three shapes Windows
//!   graphs: functions y = f(x) (or a bare f(x)), functions x = g(y), and implicit
//!   relations such as x² + y² = 25, each also as an inequality.
//! - `plot` turns them into lines and inequality regions for a view, and finds that view
//!   (Windows' automatic best fit), the grid and the point the tracer snaps to.
//! - `analysis` finds the key graph features of a function, with `exact` writing the
//!   numbers it finds as the exact values they are (`π/2`, `−2√3 − 6`).

pub mod analysis;
pub mod exact;
pub mod expr;
pub mod parse;
pub mod plot;

pub use expr::{Env, Expr};
pub use parse::{Relation, Syntax, SyntaxError};

/// How an equation relates x and y.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// y (relation) f(x).
    Function(Expr),
    /// x (relation) g(y): plotted like a function with the axes swapped.
    Inverse(Expr),
    /// F(x, y) (relation) 0.
    Implicit(Expr),
}

/// A graphable line of the equation list.
#[derive(Clone, Debug, PartialEq)]
pub struct Equation {
    pub shape: Shape,
    pub relation: Relation,
    /// The free variables (`y = mx + b` has b and m), sorted.
    pub variables: Vec<char>,
}

impl Equation {
    /// Parses a line. A bare expression is y = expression, as in Windows.
    pub fn parse(text: &str, syntax: Syntax) -> Result<Equation, SyntaxError> {
        let parsed = parse::parse(text, syntax)?;
        // y, or a function's name `f(x)`, which the parser reads as f · x. Only the name
        // counts: `ax = 3` is the line x = 3/a.
        let [left_fx, right_fx] = parsed.names_function;
        let (shape, relation) = match parsed.right {
            None if parsed.left.has_y() => return Err(SyntaxError::InvalidEquationSyntax),
            None => (Shape::Function(parsed.left), Relation::Equal),
            Some((relation, right)) => {
                let is_y = [
                    left_fx || parsed.left == Expr::Y,
                    right_fx || right == Expr::Y,
                ];
                classify(parsed.left, relation, right, is_y)?
            }
        };
        let variables = match &shape {
            Shape::Function(e) | Shape::Inverse(e) | Shape::Implicit(e) => e.variables(),
        };
        Ok(Equation {
            shape,
            relation,
            variables,
        })
    }

    pub fn is_inequality(&self) -> bool {
        self.relation != Relation::Equal
    }
}

/// Sorts `left` (relation) `right` into a shape; `is_y` tells which sides stand for y.
fn classify(
    left: Expr,
    relation: Relation,
    right: Expr,
    is_y: [bool; 2],
) -> Result<(Shape, Relation), SyntaxError> {
    if is_y[0] && !right.has_y() {
        return Ok((Shape::Function(right), relation));
    }
    if is_y[1] && !left.has_y() {
        return Ok((Shape::Function(left), relation.flip()));
    }
    if left == Expr::X && !right.has_x() {
        return Ok((Shape::Inverse(right), relation));
    }
    if right == Expr::X && !left.has_x() {
        return Ok((Shape::Inverse(left), relation.flip()));
    }
    if !(left.has_x() || left.has_y() || right.has_x() || right.has_y()) {
        return Err(SyntaxError::EqualWithoutGraphVariable);
    }
    let difference = Expr::Bin(expr::BinOp::Sub, Box::new(left), Box::new(right));
    Ok((Shape::Implicit(difference), relation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(text: &str) -> (Shape, Relation) {
        let e = Equation::parse(text, Syntax::default()).unwrap();
        (e.shape, e.relation)
    }

    #[test]
    fn shapes() {
        assert!(matches!(shape("x^2").0, Shape::Function(_)));
        assert!(matches!(shape("y=x^2").0, Shape::Function(_)));
        assert!(matches!(shape("f(x)=x^2").0, Shape::Function(_)));
        assert!(matches!(shape("3x=y").0, Shape::Function(_)));
        assert!(matches!(shape("x=3").0, Shape::Inverse(_)));
        assert!(matches!(shape("x=y^2").0, Shape::Inverse(_)));
        assert!(matches!(shape("x^2+y^2=25").0, Shape::Implicit(_)));
        assert!(matches!(shape("y=y^2+x").0, Shape::Implicit(_)));
        // `2 < y` is `y > 2`.
        assert_eq!(shape("2<y").1, Relation::Greater);
        assert_eq!(shape("y<sin(x)").1, Relation::Less);
        assert!(matches!(shape("x^2=g(x)").0, Shape::Function(_)));
    }

    #[test]
    fn a_product_with_x_is_no_function_name() {
        // `ax = 3` is a·x = 3 (the line x = 3/a), not f(x) = 3.
        let e = Equation::parse("ax=3", Syntax::default()).unwrap();
        assert!(matches!(e.shape, Shape::Implicit(_)), "{:?}", e.shape);
        assert_eq!(e.variables, vec!['a']);
        let e = Equation::parse("3=bx", Syntax::default()).unwrap();
        assert!(matches!(e.shape, Shape::Implicit(_)), "{:?}", e.shape);
        // `f(x) = x²` defines y.
        let e = Equation::parse("f(x)=x^2", Syntax::default()).unwrap();
        assert_eq!(
            e.shape,
            Equation::parse("x^2", Syntax::default()).unwrap().shape
        );
        assert!(e.variables.is_empty());
    }

    #[test]
    fn errors() {
        let error = |text| Equation::parse(text, Syntax::default()).unwrap_err();
        assert_eq!(error("x+y"), SyntaxError::InvalidEquationSyntax);
        assert_eq!(error("4+83=9"), SyntaxError::EqualWithoutGraphVariable);
        assert_eq!(error("a=5"), SyntaxError::EqualWithoutGraphVariable);
    }

    #[test]
    fn variables() {
        let e = Equation::parse("y=mx+b", Syntax::default()).unwrap();
        assert_eq!(e.variables, vec!['b', 'm']);
    }
}
