//! The content stream replay.
//!
//! Only the operators that matter for placing text are interpreted. Everything
//! else is skipped, but the graphics state stack is still tracked so that a
//! `cm` inside a `q ... Q` pair does not leak out.

use lopdf::content::{Content, Operation};
use lopdf::Object;

use super::ShowItem;
use crate::geom::Matrix;

/// Called for every text showing operator with its index in the stream, the
/// operator name, the items it shows and the state that was active.
pub(crate) type ShowCallback<'a> = dyn FnMut(usize, &str, Vec<ShowItem>, &GraphicsState) + 'a;

/// The text state, reset at every `BT`.
#[derive(Debug, Clone, PartialEq)]
pub struct TextState {
    /// Text matrix, `Tm`.
    pub matrix: Matrix,
    /// Text line matrix, `Tlm`. `Td` and `T*` move relative to this one.
    pub line_matrix: Matrix,
    pub font_resource: String,
    pub font_size: f64,
    pub char_spacing: f64,
    pub word_spacing: f64,
    /// `Tz` as a factor, so 100 percent is stored as 1.0.
    pub horizontal_scale: f64,
    pub leading: f64,
    pub rise: f64,
    pub render_mode: i64,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            matrix: Matrix::IDENTITY,
            line_matrix: Matrix::IDENTITY,
            font_resource: String::new(),
            font_size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal_scale: 1.0,
            leading: 0.0,
            rise: 0.0,
            render_mode: 0,
        }
    }
}

/// The parts of the graphics state we need. Colour, line width and clipping are
/// tracked from M5 on, when drawing and annotations arrive.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphicsState {
    pub ctm: Matrix,
    pub text: TextState,
}

fn number(operands: &[Object], index: usize) -> Option<f64> {
    operands
        .get(index)
        .and_then(|o| o.as_float().ok())
        .map(|v| v as f64)
}

fn matrix_from(operands: &[Object]) -> Option<Matrix> {
    if operands.len() < 6 {
        return None;
    }
    let mut values = [0.0f64; 6];
    for (slot, index) in values.iter_mut().zip(0..6) {
        *slot = number(operands, index)?;
    }
    Some(Matrix::from_array(values))
}

fn show_items(operation: &Operation) -> Vec<ShowItem> {
    match operation.operator.as_str() {
        "Tj" => operation
            .operands
            .first()
            .and_then(|o| o.as_str().ok())
            .map(|bytes| vec![ShowItem::Text(bytes.to_vec())])
            .unwrap_or_default(),
        "'" => operation
            .operands
            .first()
            .and_then(|o| o.as_str().ok())
            .map(|bytes| vec![ShowItem::Text(bytes.to_vec())])
            .unwrap_or_default(),
        // The two numeric operands of `"` come first, the string is last.
        "\"" => operation
            .operands
            .get(2)
            .and_then(|o| o.as_str().ok())
            .map(|bytes| vec![ShowItem::Text(bytes.to_vec())])
            .unwrap_or_default(),
        "TJ" => operation
            .operands
            .first()
            .and_then(|o| o.as_array().ok())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| match item {
                        Object::String(bytes, _) => Some(ShowItem::Text(bytes.clone())),
                        Object::Integer(value) => Some(ShowItem::Adjust(*value as f64)),
                        Object::Real(value) => Some(ShowItem::Adjust(*value as f64)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Replay one content stream, calling `on_show` for every text showing operator.
pub(crate) fn replay(
    content: &Content,
    state: &mut GraphicsState,
    stack: &mut Vec<GraphicsState>,
    on_show: &mut ShowCallback<'_>,
) {
    for (index, operation) in content.operations.iter().enumerate() {
        let operands = operation.operands.as_slice();
        match operation.operator.as_str() {
            "q" => stack.push(state.clone()),
            "Q" => {
                if let Some(previous) = stack.pop() {
                    *state = previous;
                }
            }
            "cm" => {
                if let Some(m) = matrix_from(operands) {
                    state.ctm = m.then(&state.ctm);
                }
            }
            "BT" => {
                state.text.matrix = Matrix::IDENTITY;
                state.text.line_matrix = Matrix::IDENTITY;
            }
            "ET" => {}
            "Tf" => {
                if let Some(name) = operands.first().and_then(|o| o.as_name().ok()) {
                    state.text.font_resource = String::from_utf8_lossy(name).to_string();
                }
                if let Some(size) = number(operands, 1) {
                    state.text.font_size = size;
                }
            }
            "Tc" => {
                if let Some(v) = number(operands, 0) {
                    state.text.char_spacing = v;
                }
            }
            "Tw" => {
                if let Some(v) = number(operands, 0) {
                    state.text.word_spacing = v;
                }
            }
            "Tz" => {
                if let Some(v) = number(operands, 0) {
                    state.text.horizontal_scale = v / 100.0;
                }
            }
            "TL" => {
                if let Some(v) = number(operands, 0) {
                    state.text.leading = v;
                }
            }
            "Ts" => {
                if let Some(v) = number(operands, 0) {
                    state.text.rise = v;
                }
            }
            "Tr" => {
                if let Some(v) = operands.first().and_then(|o| o.as_i64().ok()) {
                    state.text.render_mode = v;
                }
            }
            "Td" => {
                if let (Some(tx), Some(ty)) = (number(operands, 0), number(operands, 1)) {
                    next_line(state, tx, ty);
                }
            }
            "TD" => {
                if let (Some(tx), Some(ty)) = (number(operands, 0), number(operands, 1)) {
                    state.text.leading = -ty;
                    next_line(state, tx, ty);
                }
            }
            "Tm" => {
                if let Some(m) = matrix_from(operands) {
                    state.text.matrix = m;
                    state.text.line_matrix = m;
                }
            }
            "T*" => {
                let leading = state.text.leading;
                next_line(state, 0.0, -leading);
            }
            "Tj" | "TJ" => {
                let items = show_items(operation);
                on_show(index, &operation.operator, items.clone(), state);
                advance_after_show(state, &items);
            }
            "'" => {
                let leading = state.text.leading;
                next_line(state, 0.0, -leading);
                let items = show_items(operation);
                on_show(index, &operation.operator, items.clone(), state);
                advance_after_show(state, &items);
            }
            "\"" => {
                if let Some(aw) = number(operands, 0) {
                    state.text.word_spacing = aw;
                }
                if let Some(ac) = number(operands, 1) {
                    state.text.char_spacing = ac;
                }
                let leading = state.text.leading;
                next_line(state, 0.0, -leading);
                let items = show_items(operation);
                on_show(index, &operation.operator, items.clone(), state);
                advance_after_show(state, &items);
            }
            _ => {}
        }
    }
}

fn next_line(state: &mut GraphicsState, tx: f64, ty: f64) {
    let moved = Matrix::translate(tx, ty).then(&state.text.line_matrix);
    state.text.line_matrix = moved;
    state.text.matrix = moved;
}

/// Move the text matrix on after a showing operator.
///
/// Only the part that does not need the font is applied here: the explicit
/// displacements inside a `TJ` array. The glyph advance itself needs the widths
/// of the embedded font, which arrives with the font work in M2. Until then the
/// position of a run that follows another run on the same line without an
/// explicit `Td` is reported at the start of the line.
fn advance_after_show(state: &mut GraphicsState, items: &[ShowItem]) {
    let mut displacement = 0.0;
    for item in items {
        if let ShowItem::Adjust(value) = item {
            displacement -= value / 1000.0 * state.text.font_size * state.text.horizontal_scale;
        }
    }
    if displacement != 0.0 {
        state.text.matrix = Matrix::translate(displacement, 0.0).then(&state.text.matrix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::Operation;

    fn replay_ops(operations: Vec<Operation>) -> Vec<(String, Matrix, Matrix)> {
        let content = Content { operations };
        let mut state = GraphicsState::default();
        let mut stack = Vec::new();
        let mut seen = Vec::new();
        replay(&content, &mut state, &mut stack, &mut |_, op, _, state| {
            seen.push((op.to_string(), state.text.matrix, state.ctm));
        });
        seen
    }

    #[test]
    fn q_and_capital_q_restore_the_transformation() {
        let seen = replay_ops(vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![2.into(), 0.into(), 0.into(), 2.into(), 0.into(), 0.into()],
            ),
            Operation::new("BT", vec![]),
            Operation::new("Tj", vec![Object::string_literal("a")]),
            Operation::new("ET", vec![]),
            Operation::new("Q", vec![]),
            Operation::new("BT", vec![]),
            Operation::new("Tj", vec![Object::string_literal("b")]),
            Operation::new("ET", vec![]),
        ]);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].2, Matrix::scale(2.0, 2.0));
        assert_eq!(seen[1].2, Matrix::IDENTITY);
    }

    #[test]
    fn td_moves_relative_to_the_line_matrix() {
        let seen = replay_ops(vec![
            Operation::new("BT", vec![]),
            Operation::new("Td", vec![10.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal("a")]),
            Operation::new("Td", vec![0.into(), (-20).into()]),
            Operation::new("Tj", vec![Object::string_literal("b")]),
            Operation::new("ET", vec![]),
        ]);
        assert_eq!(seen[0].1, Matrix::translate(10.0, 700.0));
        assert_eq!(seen[1].1, Matrix::translate(10.0, 680.0));
    }

    #[test]
    fn t_star_uses_the_leading() {
        let seen = replay_ops(vec![
            Operation::new("BT", vec![]),
            Operation::new("TL", vec![14.into()]),
            Operation::new("Td", vec![0.into(), 100.into()]),
            Operation::new("Tj", vec![Object::string_literal("a")]),
            Operation::new("T*", vec![]),
            Operation::new("Tj", vec![Object::string_literal("b")]),
            Operation::new("ET", vec![]),
        ]);
        assert_eq!(seen[1].1, Matrix::translate(0.0, 86.0));
    }

    #[test]
    fn quote_operators_set_spacing_and_move_down() {
        let seen = replay_ops(vec![
            Operation::new("BT", vec![]),
            Operation::new("TL", vec![12.into()]),
            Operation::new("Td", vec![0.into(), 500.into()]),
            Operation::new("\"", vec![3.into(), 1.into(), Object::string_literal("x")]),
            Operation::new("ET", vec![]),
        ]);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].1, Matrix::translate(0.0, 488.0));
    }

    #[test]
    fn tj_adjustments_shift_the_text_matrix() {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 10.into()]),
                Operation::new(
                    "TJ",
                    vec![Object::Array(vec![
                        Object::string_literal("a"),
                        Object::Integer(-500),
                        Object::string_literal("b"),
                    ])],
                ),
                Operation::new("ET", vec![]),
            ],
        };
        let mut state = GraphicsState::default();
        let mut stack = Vec::new();
        replay(&content, &mut state, &mut stack, &mut |_, _, _, _| {});
        // -500 thousandths at font size 10 is half a point of extra space.
        assert!(
            (state.text.matrix.e - 5.0).abs() < 1e-9,
            "e was {}",
            state.text.matrix.e
        );
    }
}
