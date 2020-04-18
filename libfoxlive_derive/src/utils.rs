use syn;

pub fn to_camel_ident(ident: &syn::Ident) -> syn::Ident {
    let ident = ident.to_string();
    let mut out = String::with_capacity(ident.len());

    let mut iter = ident.chars();

    out.push(iter.next().unwrap().to_uppercase().next().unwrap());

    while let Some(c) = iter.next() {
        out.push(match c {
            '_' => match iter.next() {
                Some(c) => c.to_uppercase().next().unwrap(),
                None => break,
            },
            _ => c
        });
    }

    syn::parse_str::<syn::Ident>(&out).unwrap()
}


