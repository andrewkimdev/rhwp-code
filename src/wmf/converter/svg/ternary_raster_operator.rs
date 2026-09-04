use crate::wmf::converter::{
    svg::{device_context::BlitDestRect, node::Node, util::url_string, Fill},
    *,
};

#[derive(Clone, Debug, snafu::prelude::Snafu)]
pub enum TernaryRasterOperationError {
    #[snafu(display("no brush specified: {cause}"))]
    NoBrush { cause: String },
    #[snafu(display("no source bitmap specified: {cause}"))]
    NoSource { cause: String },
}

pub struct TernaryRasterOperator {
    operation: TernaryRasterOperation,
    /// [#6617] 장치 좌표로 정규화한 목적 사각형(`DeviceContext::blit_dest_rect`).
    rect: BlitDestRect,
    brush: Option<Brush>,
    source: Option<Source>,
}

enum Source {
    Bitmap16(Bitmap16),
    Bitmap(DeviceIndependentBitmap),
}

impl TernaryRasterOperator {
    pub fn new(operation: TernaryRasterOperation, rect: BlitDestRect) -> Self {
        Self {
            operation,
            rect,
            brush: None,
            source: None,
        }
    }

    /// 목적 사각형과, 뒤집힌 축이 있으면 그 축을 되돌리는 `transform`.
    ///
    /// [#6617] 어느 축이 뒤집히는지는 논리 폭/높이 부호가 아니라 장치 좌표에서 정한다
    /// (`DeviceContext::blit_dest_rect`) — GDI 의 뒤집기는 논리 부호 × 창 축 방향의
    /// 곱이므로, 사각형 자체는 항상 양수 크기로 두고 뒤집힘은 요소 자신의 `transform`
    /// 으로 표현한다.
    fn normalized_rect(&self) -> (i32, i32, i32, i32, Option<String>) {
        let BlitDestRect {
            x,
            y,
            width,
            height,
            flip_x,
            flip_y,
        } = self.rect;
        let mut parts = Vec::new();
        if flip_x {
            parts.push(format!("translate({},0) scale(-1,1)", 2 * x + width));
        }
        if flip_y {
            parts.push(format!("translate(0,{}) scale(1,-1)", 2 * y + height));
        }
        let transform = (!parts.is_empty()).then(|| parts.join(" "));
        (x, y, width, height, transform)
    }

    pub fn brush(mut self, brush: Brush) -> Self {
        self.brush = brush.into();
        self
    }

    pub fn source_bitmap16(mut self, source: Bitmap16) -> Self {
        self.source = Source::Bitmap16(source).into();
        self
    }

    pub fn source_bitmap(mut self, source: DeviceIndependentBitmap) -> Self {
        self.source = Source::Bitmap(source).into();
        self
    }

    pub fn run(
        self,
        definitions: &mut Vec<Node>,
    ) -> Result<Option<Node>, TernaryRasterOperationError> {
        if self.operation.use_selected_brush() && self.brush.is_none() {
            return Err(TernaryRasterOperationError::NoBrush {
                cause: format!(
                    "TernaryRasterOperation {:?} cannot access brush.",
                    self.operation,
                ),
            });
        }

        if self.operation.use_source() && self.source.is_none() {
            return Err(TernaryRasterOperationError::NoSource {
                cause: format!(
                    "TernaryRasterOperation {:?} cannot access source bitmap.",
                    self.operation,
                ),
            });
        }

        let (x, y, width, height, transform) = self.normalized_rect();

        let mut result: Node = match self.operation {
            TernaryRasterOperation::BLACKNESS => Node::new("rect")
                .set("x", x)
                .set("y", y)
                .set("width", width)
                .set("height", height)
                .set("stroke", "none")
                .set("fill", "black"),
            TernaryRasterOperation::SRCCOPY => {
                let bitmap = match self.source.unwrap() {
                    Source::Bitmap16(data) => {
                        let bitmap = crate::wmf::parser::DeviceIndependentBitmap::from(data);
                        crate::wmf::converter::Bitmap::from(bitmap)
                    }
                    Source::Bitmap(data) => Bitmap::from(data),
                };

                Node::new("image")
                    .set("x", x)
                    .set("y", y)
                    .set("width", width)
                    .set("height", height)
                    .set("href", bitmap.as_data_url())
            }
            TernaryRasterOperation::PATCOPY => {
                let fill = match Fill::from(self.brush.clone().unwrap()) {
                    Fill::Pattern { pattern } => {
                        let id = Self::issue_id(definitions);
                        definitions.push(pattern.set("id", id.as_str()));
                        url_string(format!("#{id}").as_str())
                    }
                    Fill::Value { value } => value,
                };

                Node::new("rect")
                    .set("x", x)
                    .set("y", y)
                    .set("width", width)
                    .set("height", height)
                    .set("fill", fill.as_str())
            }
            TernaryRasterOperation::WHITENESS => Node::new("rect")
                .set("x", x)
                .set("y", y)
                .set("width", width)
                .set("height", height)
                .set("stroke", "none")
                .set("fill", "white"),
            operation => {
                info!(?operation, "TernaryRasterOperation is not implemented");

                return Ok(None);
            }
        };

        if let Some(transform) = transform {
            result = result.set("transform", transform);
        }

        Ok(Some(result))
    }

    #[inline]
    fn issue_id(definitions: &[Node]) -> String {
        format!("rop_pat{}", definitions.len())
    }
}

impl From<ColorRef> for RGBQuad {
    fn from(v: ColorRef) -> Self {
        let ColorRef {
            red,
            green,
            blue,
            reserved,
        } = v;
        Self {
            red,
            green,
            blue,
            reserved,
        }
    }
}
