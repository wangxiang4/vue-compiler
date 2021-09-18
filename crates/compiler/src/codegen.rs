use super::converter::{BaseConvertInfo, BaseRoot, ConvertInfo, IRNode, IRRoot, JsExpr as Js};
use super::util::VStr;
use rustc_hash::FxHashSet;
use smallvec::{smallvec, SmallVec};
use std::borrow::Cow;
use std::fmt;
use std::io;
use std::marker::PhantomData;

pub trait CodeGenerator {
    type IR;
    type Output;
    /// generate will take optimized ir node and output
    /// desired code format, either String or Binary code
    fn generate(&mut self, node: Self::IR) -> Self::Output;
}

pub struct CodeGenerateOption {
    pub is_ts: bool,
    pub source_map: bool,
    // filename for source map
    pub filename: String,
    pub decode_entities: EntityDecoder,
}
impl Default for CodeGenerateOption {
    fn default() -> Self {
        Self {
            is_ts: false,
            source_map: false,
            filename: String::new(),
            decode_entities: |s, _| DecodedStr::from(s),
        }
    }
}

use super::converter as C;
trait CoreCodeGenerator<T: ConvertInfo>: CodeGenerator<IR = IRRoot<T>> {
    type Written;
    fn generate_prologue(&mut self, t: &IRRoot<T>) -> Self::Written;
    fn generate_epilogue(&mut self) -> Self::Written;
    fn generate_text(&mut self, t: T::TextType) -> Self::Written;
    fn generate_if(&mut self, i: C::IfNodeIR<T>) -> Self::Written;
    fn generate_for(&mut self, f: C::ForNodeIR<T>) -> Self::Written;
    fn generate_vnode(&mut self, v: C::VNodeIR<T>) -> Self::Written;
    fn generate_slot_outlet(&mut self, r: C::RenderSlotIR<T>) -> Self::Written;
    fn generate_v_slot(&mut self, s: C::VSlotIR<T>) -> Self::Written;
    fn generate_js_expr(&mut self, e: T::JsExpression) -> Self::Written;
    fn generate_comment(&mut self, c: T::CommentType) -> Self::Written;
}

struct CodeWriter<'a, T: io::Write> {
    writer: T,
    option: CodeGenerateOption,
    indent_level: usize,
    closing_brackets: usize,
    p: PhantomData<&'a ()>,
}
impl<'a, T: io::Write> CodeGenerator for CodeWriter<'a, T> {
    type IR = BaseRoot<'a>;
    type Output = io::Result<()>;
    fn generate(&mut self, root: Self::IR) -> Self::Output {
        self.generate_root(root)
    }
}

type BaseIf<'a> = C::IfNodeIR<BaseConvertInfo<'a>>;
type BaseFor<'a> = C::ForNodeIR<BaseConvertInfo<'a>>;
type BaseVNode<'a> = C::VNodeIR<BaseConvertInfo<'a>>;
type BaseRenderSlot<'a> = C::RenderSlotIR<BaseConvertInfo<'a>>;
type BaseVSlot<'a> = C::VSlotIR<BaseConvertInfo<'a>>;

impl<'a, T: io::Write> CoreCodeGenerator<BaseConvertInfo<'a>> for CodeWriter<'a, T> {
    type Written = io::Result<()>;
    fn generate_prologue(&mut self, root: &BaseRoot<'a>) -> io::Result<()> {
        self.generate_preamble()?;
        self.generate_function_signature()?;
        self.generate_with_block()?;
        self.generate_assets()?;
        self.writer.write_all(b"return ")
    }
    fn generate_epilogue(&mut self) -> io::Result<()> {
        for _ in 0..self.closing_brackets {
            self.deindent(true)?;
            self.writer.write_all(b"}")?;
        }
        debug_assert_eq!(self.indent_level, 0);
        Ok(())
    }
    fn generate_text(&mut self, t: SmallVec<[Js<'a>; 1]>) -> io::Result<()> {
        let mut texts = t.into_iter();
        match texts.next() {
            Some(t) => self.generate_js_expr(t)?,
            None => return Ok(()),
        }
        for t in texts {
            self.writer.write_all(b" + ")?;
            self.generate_js_expr(t)?;
        }
        Ok(())
    }
    fn generate_if(&mut self, i: BaseIf<'a>) -> io::Result<()> {
        todo!()
    }
    fn generate_for(&mut self, f: BaseFor<'a>) -> io::Result<()> {
        todo!()
    }
    fn generate_vnode(&mut self, v: BaseVNode<'a>) -> io::Result<()> {
        todo!()
    }
    fn generate_slot_outlet(&mut self, r: BaseRenderSlot<'a>) -> io::Result<()> {
        todo!()
    }
    fn generate_v_slot(&mut self, s: BaseVSlot<'a>) -> io::Result<()> {
        todo!()
    }
    fn generate_js_expr(&mut self, expr: Js<'a>) -> io::Result<()> {
        match expr {
            Js::Src(s) => self.writer.write_all(s.as_bytes()),
            Js::StrLit(mut l) => l.be_js_str().write_to(&mut self.writer),
            Js::Simple(e, _) => e.write_to(&mut self.writer),
            Js::Symbol(s) => {
                self.writer.write_all(b"_")?;
                self.writer.write_all(s.helper_str().as_bytes())
            }
            Js::Props(p) => {
                todo!()
            }
            Js::Compound(v) => {
                for e in v {
                    self.generate_js_expr(e)?;
                }
                Ok(())
            }
            Js::Array(a) => {
                self.writer.write_all(b"[")?;
                self.gen_comma_separated(a)?;
                self.writer.write_all(b"]")
            }
            Js::Call(c, args) => {
                self.writer.write_all(b"_")?;
                self.writer.write_all(c.helper_str().as_bytes())?;
                self.writer.write_all(b"(")?;
                self.gen_comma_separated(args)?;
                self.writer.write_all(b")")
            }
        }
    }
    fn generate_comment(&mut self, c: &'a str) -> io::Result<()> {
        todo!()
    }
}

impl<'a, T: io::Write> CodeWriter<'a, T> {
    fn generate_root(&mut self, root: BaseRoot<'a>) -> io::Result<()> {
        use IRNode as IR;
        self.generate_prologue(&root)?;
        if root.body.is_empty() {
            self.writer.write_all(b"null")?;
        } else {
            for node in root.body {
                match node {
                    IR::TextCall(t) => self.generate_text(t)?,
                    IR::If(v_if) => self.generate_if(v_if)?,
                    IR::For(v_for) => self.generate_for(v_for)?,
                    IR::VNodeCall(vnode) => self.generate_vnode(vnode)?,
                    IR::RenderSlotCall(r) => self.generate_slot_outlet(r)?,
                    IR::VSlotUse(s) => self.generate_v_slot(s)?,
                    IR::CommentCall(c) => self.generate_comment(c)?,
                    IR::GenericExpression(e) => self.generate_js_expr(e)?,
                    IR::AlterableSlot(..) => {
                        panic!("alterable slot should be compiled");
                    }
                };
            }
        }
        self.generate_epilogue()
    }
    /// for import helpers or hoist that not in function
    fn generate_preamble(&mut self) -> io::Result<()> {
        self.writer.write_all(b"return ")
    }
    /// render() or ssrRender() or IIFE for inline mode
    fn generate_function_signature(&mut self) -> io::Result<()> {
        // TODO: add more params, add more modes
        self.writer.write_all(b"function render(_ctx, _cache) {")?;
        self.closing_brackets += 1;
        self.indent()
    }
    /// with (ctx) for not prefixIdentifier
    fn generate_with_block(&mut self) -> io::Result<()> {
        // TODO: add helpers
        self.writer.write_all(b"with (_ctx) {")?;
        self.closing_brackets += 1;
        self.indent()
    }
    /// component/directive resolotuion inside render
    fn generate_assets(&mut self) -> io::Result<()> {
        // TODO
        Ok(())
    }
    fn gen_comma_separated(&mut self, exprs: Vec<Js<'a>>) -> io::Result<()> {
        let mut exprs = exprs.into_iter();
        if let Some(e) = exprs.next() {
            self.generate_js_expr(e)?;
        } else {
            return Ok(());
        }
        for e in exprs {
            self.writer.write_all(b", ")?;
            self.generate_js_expr(e)?;
        }
        Ok(())
    }

    fn newline(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\n")?;
        for _ in 0..self.indent_level {
            self.writer.write_all(b"  ")?;
        }
        Ok(())
    }
    fn indent(&mut self) -> io::Result<()> {
        self.indent_level += 1;
        self.newline()
    }
    fn deindent(&mut self, with_new_line: bool) -> io::Result<()> {
        self.indent_level -= 1;
        if with_new_line {
            self.newline()
        } else {
            Ok(())
        }
    }
}

pub trait CodeGenWrite: fmt::Write {}

/// DecodedStr represents text after decoding html entities.
/// SmallVec and Cow are used internally for less allocation.
#[derive(Debug)]
pub struct DecodedStr<'a>(SmallVec<[Cow<'a, str>; 1]>);

impl<'a> From<&'a str> for DecodedStr<'a> {
    fn from(decoded: &'a str) -> Self {
        debug_assert!(!decoded.is_empty());
        Self(smallvec![Cow::Borrowed(decoded)])
    }
}

pub type EntityDecoder = fn(&str, bool) -> DecodedStr<'_>;

fn stringify_dynamic_prop_names(prop_names: FxHashSet<VStr>) -> Option<Js> {
    todo!()
}

#[cfg(test)]
mod test {
    use super::super::converter::test::base_convert;
    use super::*;
    fn base_gen(s: &str) -> String {
        let mut writer = CodeWriter {
            writer: vec![],
            option: CodeGenerateOption::default(),
            indent_level: 0,
            closing_brackets: 0,
            p: PhantomData,
        };
        let ir = base_convert(s);
        writer.generate_root(ir).unwrap();
        String::from_utf8(writer.writer).unwrap()
    }
    #[test]
    fn test_text() {
        let s = base_gen("hello       world");
        assert!(s.contains(stringify!("hello world")));
        // let s = base_gen("hello {{world}}");
        // assert!(s.contains("\"hello\" + world"), "{}", s);
    }
}
