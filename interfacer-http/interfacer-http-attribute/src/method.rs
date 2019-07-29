use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, TraitItemMethod};

use crate::attr::{Attr, AttrExpr};
use crate::format_uri::gen_uri_format_expr;
use proc_macro::Diagnostic;

pub fn transform_method(raw_method: &mut TraitItemMethod) -> Result<(), Diagnostic> {
    let args = Attr::from_raw(raw_method)?;
    let import_stmt = import();
    let define_final_uri = gen_final_uri(&args)?;
    let define_expect_content_type = gen_expect_content_type(&args);
    let define_request = build_request(&args, &raw_method);
    let send_request_stmt = send_request();
    let check_response_stmt = check_response(&args);
    let ret = ret();
    let body = quote!(
        #import_stmt
        #define_final_uri
        #define_expect_content_type
        #define_request
        #send_request_stmt
        #check_response_stmt
        #ret
    );
    raw_method.semi_token = None;
    raw_method.default = Some(parse_quote!({
        #body
    }));
    Ok(())
}

macro_rules! use_idents {
    ($($idents:ident),*) => {
        $(let $idents = quote!($idents);)*
    };
}

fn import() -> TokenStream {
    quote!(
        use interfacer_http::{
            RequestFail, ContentType,
            http::{StatusCode, header::CONTENT_TYPE, Response},
            IntoStruct, ToContent, HttpClient, StringError,
        };
        use std::convert::TryInto;
    )
}

fn gen_final_uri(args: &Attr) -> Result<TokenStream, Diagnostic> {
    use_idents!(final_uri_ident);
    let uri_format_expr = gen_uri_format_expr(&args.req.path)?;
    Ok(quote!(
        let #final_uri_ident = self.get_base_url().join(&#uri_format_expr)?;
    ))
}

fn gen_expect_content_type(args: &Attr) -> TokenStream {
    use_idents!(expect_content_type_ident);
    let expect_content_type = match &args.expect.content_type {
        AttrExpr::Lit(lit) => quote!(#lit), // TODO: check lit
        AttrExpr::Path(path) => quote!(#path),
    };
    quote!(
        let #expect_content_type_ident = #expect_content_type.try_into()?;
    )
}

fn send_request() -> TokenStream {
    use_idents!(request_ident, parts_ident, body_ident);
    quote!(
        let (#parts_ident, #body_ident) = self.get_client().request(#request_ident).await.map_err(|err| err.into())?.into_parts();
    )
}

fn check_response(args: &Attr) -> TokenStream {
    use_idents!(parts_ident, expect_content_type_ident);
    let expect_status = match &args.expect.status {
        AttrExpr::Lit(lit) => quote!(#lit), // TODO: check lit
        AttrExpr::Path(path) => quote!(#path),
    };
    quote!(
        RequestFail::expect_status(StatusCode::from_u16(#expect_status).unwrap(), #parts_ident.status)?;
        let ret_content_type = parts_ident.headers.get(CONTENT_TYPE).ok_or(
            StringError::new("cannot get Content-Type from response headers")
        )?;
        #expect_content_type_ident.expect(&ContentType::from_header(ret_content_type)?)?;
    )
}

fn ret() -> TokenStream {
    use_idents!(body_ident, expect_content_type_ident, parts_ident);
    quote!(
        Ok(
            Response::from_parts(
                #parts_ident,
                #body_ident.into_struct(&#expect_content_type_ident)?,
            ).into(),
        )
    )
}

// TODO: complete build request; using generic Body type
fn build_request(args: &Attr, _raw_method: &TraitItemMethod) -> TokenStream {
    let method = args.req.method.as_str();
    use_idents!(request_ident, final_uri_ident);
    quote!(
        let mut builder = interfacer_http::http::Request::builder();
        let #request_ident = builder
            .uri(#final_uri_ident.as_str())
            .method(#method)
            .body(Vec::new())?;
    )
}
