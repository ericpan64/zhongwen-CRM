/*
/// Route handling for authenticated users.
/// Expected form inputs are stored as Structs and defined above the corresponding route.
/// 
/// users.rs
/// ├── GET
/// |   └── /u/<username>
/// |   └── /u/<username>/<doc_title>
/// |   └── /api/delete-doc/<doc_title>
/// |   └── /api/delete-vocab/<phrase>
/// |   └── /api/logout
/// |   └── /api/docs-to-csv
/// |   └── /api/vocab-to-csv
/// |
/// └── POST
///     └── /api/login
///     └── /api/register
///     └── /api/upload
///     └── /api/url-upload
///     └── /api/vocab
///     └── /api/update-settings
*/

use crate::{
    convert_rawstr_to_string, 
    DatabaseItem,
    auth::{generate_http_cookie, get_username_from_cookie},
    config::JWT_NAME,
    html as html_rendering,
    models::{
        user::{User, UserDoc, UserVocab, UserVocabList},
        zh::{CnType, CnPhonetics}
    }
};
use mongodb::{
    bson::doc,
    sync::Database
};
use rocket::{
    http::{RawStr, Cookie, Cookies, Status},
    request::Form,
    response::Redirect,
    State,
};
use rocket_contrib::{
    json::Json,
    templates::Template
};
use serde::Serialize;
use std::collections::HashMap;
use tokio::runtime::Handle;

/* GET */
/// /u/<raw_username>
#[get("/u/<raw_username>")]
pub fn user_profile(cookies: Cookies, db: State<Database>, raw_username: &RawStr) -> Template {
    let mut context: HashMap<&str, String> = HashMap::new(); // Note: <&str, String> makes more sense than <&str, &str> due to variable lifetimes
    let username = convert_rawstr_to_string(raw_username);
    // Compare username with logged-in username from JWT
    match get_username_from_cookie(&db, cookies.get(JWT_NAME)) {
        Some(s) => { 
            if &s == &username {
                let (cn_type, cn_phonetics) = User::get_user_settings(&db, &username);
                let doc_html = html_rendering::render_document_table(&db, &username);
                let vocab_html = html_rendering::render_vocab_table(&db, &username);
            
                context.insert("doc_table", doc_html);
                context.insert("vocab_table", vocab_html);
                context.insert("cn_type", cn_type.to_string());
                context.insert("cn_phonetics", cn_phonetics.to_string());           
            }
            context.insert("logged_in_username", s);
        },
        None => { }
    }
    if User::check_if_username_exists(&db, &username) == true {
        context.insert("username", username); 
    }
    return Template::render("userprofile", context);
}
/// /u/<raw_username>/<doc_title>
#[get("/u/<raw_username>/<doc_title>")]
pub fn user_view_doc(cookies: Cookies, db: State<Database>, raw_username: &RawStr, doc_title: &RawStr) -> Template {
    let mut context: HashMap<&str, String> = HashMap::new(); // Note: <&str, String> makes more sense than <&str, &str> due to variable lifetimes
    let username = convert_rawstr_to_string(raw_username);
    // Compare username with logged-in username from JWT
    match get_username_from_cookie(&db, cookies.get(JWT_NAME)) {
        Some(s) => { 
            if &s == &username {
                // Get html to render
                let (cn_type, cn_phonetics) = User::get_user_settings(&db, &username);
                let title = convert_rawstr_to_string(doc_title);
                let doc_html_res = UserDoc::get_values_from_query(&db, 
                    doc!{ "username": &username, "title": &title},
                    vec!["body_html"])[0].to_owned();
                let query_res = UserVocabList::get_values_from_query(&db, 
                    doc! { "username": &username, "cn_type": cn_type.as_str() },
                    vec!["unique_char_list", "unique_uid_list"]);
                let user_char_list_string = query_res[0].to_owned();
                let user_uid_list_string = query_res[1].to_owned();
                context.insert("paragraph_html", doc_html_res);
                context.insert("user_char_list_string", user_char_list_string);
                context.insert("user_uid_list_string", user_uid_list_string);
                context.insert("cn_phonetics", cn_phonetics.to_string());
            }
        },
        None =>  { context.insert("paragraph_html", String::from("<p>Not authenticated as user</p>")); }
    }
    if User::check_if_username_exists(&db, &username) == true {
        context.insert("username", username); 
    }
    return Template::render("reader", context);
}
/// /api/delete-doc/<doc_title>
#[get("/api/delete-doc/<doc_title>")]
pub fn delete_user_doc(cookies: Cookies, db: State<Database>, rt: State<Handle>, doc_title: &RawStr) -> Redirect {
    let title = convert_rawstr_to_string(doc_title);
    let username = get_username_from_cookie(&db, cookies.get(JWT_NAME)).unwrap();
    rt.block_on(UserDoc::try_delete(&db, &username, &title));
    return Redirect::to(uri!(user_profile: username));
}
/// /api/delete-vocab/<vocab_uid>
#[get("/api/delete-vocab/<vocab_uid>")]
pub fn delete_user_vocab(cookies: Cookies, db: State<Database>, rt: State<Handle>, vocab_uid: &RawStr) -> Redirect {
    let phrase_uid = convert_rawstr_to_string(vocab_uid);
    let username = get_username_from_cookie(&db, cookies.get(JWT_NAME)).unwrap();
    let (cn_type, _) = User::get_user_settings(&db, &username);
    rt.block_on(UserVocab::try_delete(&db, &username, &phrase_uid, &cn_type));
    return Redirect::to(uri!(user_profile: username));
}
/// /api/logout
#[get("/api/logout")]
pub fn logout_user(mut cookies: Cookies) -> Redirect {
    let mut removal_cookie = Cookie::named(JWT_NAME);
    removal_cookie.set_path("/");
    cookies.remove(removal_cookie);
    return Redirect::to("/");
}
/// Matches definition in handleTables.js (called in userprofile.html.tera).
#[derive(Serialize)]
pub struct UserDocCsvList {
    title: Vec<String>,
    body: Vec<String>,
    source: Vec<String>,
    created_on: Vec<String>
}
/// /api/docs-to-csv
#[get("/api/docs-to-csv")]
pub fn documents_to_csv_json(cookies: Cookies, db: State<Database>) -> Json<UserDocCsvList> {
    let query_doc = match get_username_from_cookie(&db, cookies.get(JWT_NAME)) {
        Some(username) => {
            let (cn_type, cn_phonetics) = User::get_user_settings(&db, &username);
            doc! { "username": username, "cn_type": cn_type.as_str(), "cn_phonetics": cn_phonetics.as_str() }
        },
        None => doc! { "username": "" }
    };
    // upper-bound at 2500 docs (approx match with <=5MB csv limit), update as needed
    let fields: Vec<&str> = vec!["title", "body", "source", "created_on"];
    let field_vals = UserDoc::aggregate_all_values_from_query(&db, query_doc, fields);
    let csv_list = UserDocCsvList {
        title: field_vals[0].to_owned(),
        body: field_vals[1].to_owned(),
        source: field_vals[2].to_owned(),
        created_on: field_vals[3].to_owned()
    };
    return Json(csv_list);
}
/// Matches definition in handleTables.js (called in userprofile.html.tera).
#[derive(Serialize)]
pub struct UserVocabCsvList {
    phrase: Vec<String>,
    phrase_phonetics: Vec<String>,
    def: Vec<String>,
    from_doc_title: Vec<String>,
    radical_map: Vec<String>,
    created_on: Vec<String>
}
/// /api/vocab-to-csv
#[get("/api/vocab-to-csv")]
pub fn vocab_to_csv_json(cookies: Cookies, db: State<Database>) -> Json<UserVocabCsvList> {
    let query_doc = match get_username_from_cookie(&db, cookies.get(JWT_NAME)) {
        Some(username) => {
            let (cn_type, cn_phonetics) = User::get_user_settings(&db, &username);
            doc! { "username": username, "cn_type": cn_type.as_str(), "cn_phonetics": cn_phonetics.as_str() }
        },
        None => doc! { "username": "" }
    };
    let fields: Vec<&str> = vec!["phrase", "phrase_phonetics", "def", "from_doc_title", "radical_map", "created_on"];
    let field_vals = UserVocab::aggregate_all_values_from_query(&db, query_doc, fields);
    let csv_list = UserVocabCsvList {
        phrase: field_vals[0].to_owned(),
        phrase_phonetics: field_vals[1].to_owned(),
        def: field_vals[2].to_owned(),
        from_doc_title: field_vals[3].to_owned(),
        radical_map: field_vals[4].to_owned(),
        created_on: field_vals[5].to_owned()
    };
    return Json(csv_list);
}

/* POST */
/// Matches definition in login.html.tera.
#[derive(FromForm)]
pub struct UserLoginForm<'f> {
    username: &'f RawStr,
    password: &'f RawStr,
}
/// /api/login
#[post("/api/login", data = "<user_input>")]
pub fn login_form(mut cookies: Cookies, db: State<Database>, user_input: Form<UserLoginForm<'_>>) -> Status {
    let UserLoginForm { username, password } = user_input.into_inner();
    let username = convert_rawstr_to_string(username);
    let password = convert_rawstr_to_string(password);
    let is_valid_password = User::check_password(&db, &username, &password);
    let res_status = match is_valid_password {
        true => {
            let new_cookie = generate_http_cookie(&db, username, password);
            cookies.add(new_cookie);
            Status::Accepted
        },
        false => Status::Unauthorized
    };
    return res_status;
}
/// Matches definition in login.html.tera.
#[derive(FromForm)]
pub struct UserRegisterForm<'f> {
    username: &'f RawStr,
    email: &'f RawStr,
    password: &'f RawStr,
}
/// /api/register
#[post("/api/register", data = "<user_input>")]
pub fn register_form(mut cookies: Cookies, db: State<Database>, user_input: Form<UserRegisterForm<'_>>) -> Status {
    let UserRegisterForm { username, email, password } = user_input.into_inner();
    let username = convert_rawstr_to_string(username);
    let password = convert_rawstr_to_string(password);
    let email = convert_rawstr_to_string(email);
    let new_user = User::new(username.clone(), password.clone(), email);
    let res_status = match new_user.try_insert(&db) {
        Ok(_) => {
            let new_cookie = generate_http_cookie(&db, username, password);
            cookies.add(new_cookie);
            Status::Accepted
        },
        Err(_) => Status::UnprocessableEntity
    };
    return res_status;
}
/// Matches definition in userprofile.html.tera.
#[derive(FromForm)]
pub struct UserDocumentForm<'f> {
    title: &'f RawStr,
    source: &'f RawStr,
    body: &'f RawStr,
}
/// /api/upload
#[post("/api/upload", data="<user_doc>")]
pub fn user_doc_upload(cookies: Cookies, db: State<Database>, rt: State<Handle>, user_doc: Form<UserDocumentForm<'_>>) -> Redirect {
    let UserDocumentForm { title, source, body } = user_doc.into_inner();
    let title = convert_rawstr_to_string(title);
    let body = convert_rawstr_to_string(body);
    let source = convert_rawstr_to_string(source);    
    let username_from_cookie = get_username_from_cookie(&db, cookies.get(JWT_NAME));
    let res_redirect = match username_from_cookie {
        Some(username) => { 
            let new_doc = rt.block_on(UserDoc::new(&db, username, title, body, source));
            match new_doc.try_insert(&db) {
                Ok(username) => Redirect::to(uri!(user_profile: username)),
                Err(e) => {
                    eprintln!("Exception when inserting doc: {:?}", e);
                    Redirect::to("/")
                }
            }
        },
        None => Redirect::to("/")
    };
    return res_redirect;
}
/// Matches definition in userprofile.html.tera.
#[derive(FromForm)]
pub struct UserUrlForm<'f> {
    url: &'f RawStr,
}
/// /api/url-upload
#[post("/api/url-upload", data = "<user_url>")]
pub fn user_url_upload(cookies: Cookies, db: State<Database>, rt: State<Handle>, user_url: Form<UserUrlForm<'_>>) -> Redirect {
    let UserUrlForm { url } = user_url.into_inner();
    let url = convert_rawstr_to_string(url);
    let username_from_cookie = get_username_from_cookie(&db, cookies.get(JWT_NAME));
    let res_redirect = match username_from_cookie {
        Some(username) => { 
            let new_doc = rt.block_on(UserDoc::from_url(&db, username, url));
            match new_doc.try_insert(&db) {
                Ok(username) => Redirect::to(uri!(user_profile: username)),
                Err(e) => { 
                    eprintln!("Exception when inserting doc from url: {:?}", e);
                    Redirect::to("/") 
                } 
            }
        },
        None => Redirect::to("/")
    };
    return res_redirect;
}
/// Matches definition in template.js (primarily called in reader.html.tera).
#[derive(FromForm)]
pub struct UserVocabForm<'f> {
    phrase_uid: &'f RawStr,
    from_doc_title: &'f RawStr,
}
/// /api/vocab
#[post("/api/vocab", data="<user_vocab>")]
pub fn user_vocab_upload(cookies: Cookies, db: State<Database>, rt: State<Handle>, user_vocab: Form<UserVocabForm<'_>>) -> Status {
    let UserVocabForm { phrase_uid, from_doc_title } = user_vocab.into_inner();
    let phrase = convert_rawstr_to_string(phrase_uid);
    let from_doc_title = convert_rawstr_to_string(from_doc_title);
    let username_from_cookie = get_username_from_cookie(&db, cookies.get(JWT_NAME));
    let res_status = match username_from_cookie {
        Some(username) => { 
            let new_vocab = rt.block_on(UserVocab::new(&db, username, phrase, from_doc_title));
            match new_vocab.try_insert(&db) {
                Ok(_) => Status::Accepted,
                Err(_) => Status::ExpectationFailed
            }
        },
        None => {
            println!("Error: no username found from cookie");
            Status::BadRequest
        }
    };
    return res_status;
}
/// Matches definition in userprofile.html.tera.
#[derive(FromForm)]
pub struct UserSettingForm<'f> {
    setting: &'f RawStr,
}
/// /api/update-settings
#[post("/api/update-settings", data = "<user_setting>")]
pub fn update_settings(cookies: Cookies, db: State<Database>, user_setting: Form<UserSettingForm<'_>>) -> Status {
    let UserSettingForm { setting } = user_setting.into_inner();
    let setting = convert_rawstr_to_string(setting);
    let username_from_cookie = get_username_from_cookie(&db, cookies.get(JWT_NAME));
    let res_status = match username_from_cookie {
        Some(username) => {
            let cn_type = CnType::from_str(&setting);
            let cn_phonetics = CnPhonetics::from_str(&setting);
            match User::update_user_settings(&db, &username, cn_type, cn_phonetics) {
                Ok(_) => Status::Accepted,
                Err(_) => Status::BadRequest
            }
        },
        None => Status::Unauthorized
    };
    return res_status;
}