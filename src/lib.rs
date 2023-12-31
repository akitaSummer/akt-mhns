#![feature(async_closure)]
#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

#[macro_use]
extern crate lazy_static;

use std::convert::{Infallible, TryInto};
use std::sync::Arc;

use hyper::body::HttpBody;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use napi::{
  self,
  threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
  CallContext, Error as NapiError, JsFunction, JsNumber, JsObject, Result as JsResult, Status,
};
use rand::Rng;
use tokio::sync::Mutex;

#[cfg(all(
  target_arch = "x86_64",
  not(target_env = "musl"),
  not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

// #[module_exports]
// fn init(mut exports: JsObject) -> JsResult<()> {
//   exports.create_named_method("createApp", create_app)?;
//   Ok(())
// }
type CallbackFn = ThreadsafeFunction<Request<Body>>;

pub struct Cached<T> {
  cached: T,
}

impl<T> Cached<T> {
  pub fn new(cached: T) -> Self {
    Self { cached }
  }
}

lazy_static! {
  static ref CALLBACKLIST: Arc<Mutex<Vec<CallbackFn>>> =
    Arc::new(Mutex::new(Vec::<CallbackFn>::new()));
}

#[module_exports]
fn init(mut exports: JsObject) -> JsResult<()> {
  exports.create_named_method("createApp", create_app)?;
  exports.create_named_method("addCallback", add_callback)?;
  Ok(())
}

#[js_function(3)]
fn create_app(ctx: CallContext) -> JsResult<JsObject> {
  let port: u32 = ctx.get::<JsNumber>(0)?.try_into()?;
  let ready_callback = ctx.get::<JsFunction>(1)?;
  let on_req_callback = ctx.get::<JsFunction>(2)?;
  let ready_tsfn_callback = ctx.env.create_threadsafe_function(
    &ready_callback,
    1,
    |cx: napi::threadsafe_function::ThreadSafeCallContext<Option<u32>>| {
      cx.env.get_boolean(true).map(|v| vec![v])
    },
  )?;
  let req_tsfn_callback = ctx.env.create_threadsafe_function(
    &on_req_callback,
    1,
    |cx: napi::threadsafe_function::ThreadSafeCallContext<Request<Body>>| {
      let (parts, body) = cx.value.into_parts();
      let version = format!("{:?}", &parts.version);
      let method = parts.method.as_str();
      let uri = format!("{}", &parts.uri);
      let headers = format!("{:?}", &parts.headers);
      let body_size_hint = body.size_hint().upper().map(|s| s as i64);
      let body = cx.env.create_external(body, body_size_hint)?;
      Ok(vec![
        cx.env.create_string(&version)?.into_unknown(),
        cx.env.create_string(method)?.into_unknown(),
        cx.env.create_string(&uri)?.into_unknown(),
        cx.env.create_string(&headers)?.into_unknown(),
        body.into_unknown(),
      ])
    },
  )?;
  let tsfn_for_err = ready_tsfn_callback.clone();
  let start = async move {
    let addr = ([127, 0, 0, 1], port as _).into();
    {
      let req_tsfn_callback = req_tsfn_callback.clone();
      let arr = CALLBACKLIST.clone();
      let mut list = arr.lock().await;

      list.push(req_tsfn_callback.clone());
    }

    let make_svc = make_service_fn(move |_conn| {
      let arr = CALLBACKLIST.clone();
      async move {
        let list = arr.lock().await;
        let mut rng = rand::thread_rng();
        let random_index = rng.gen_range(0..list.len());
        let random_element = list[random_index].clone();
        Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
          // let req_tsfn_callback = req_tsfn_callback.clone();
          on_req(req, random_element.clone())
        }))
      }
    });
    let server = Server::bind(&addr).serve(make_svc);

    ready_tsfn_callback.call(
      Ok(None),
      napi::threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
    );
    server.await.map_err(move |e| {
      let err = NapiError::new(Status::GenericFailure, format!("{}", e));
      tsfn_for_err.call(
        Err(err),
        napi::threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
      );
      NapiError::new(Status::GenericFailure, format!("{}", e))
    })?;

    Ok(())
  };
  ctx
    .env
    .execute_tokio_future(start, |env, _| env.get_undefined())
}

#[inline(always)]
async fn on_req(
  req: Request<Body>,
  callback: ThreadsafeFunction<Request<Body>>,
) -> Result<Response<Body>, Infallible> {
  callback.call(Ok(req), ThreadsafeFunctionCallMode::NonBlocking);

  Ok(Response::new(Body::from("Hello!")))
}

#[js_function(1)]
pub fn add_callback(ctx: CallContext) -> JsResult<JsObject> {
  let on_req_callback = ctx.get::<JsFunction>(0)?;
  let req_tsfn_callback = on_req_callback.create_threadsafe_function(
    1,
    |cx: napi::threadsafe_function::ThreadSafeCallContext<Request<Body>>| {
      let (parts, body) = cx.value.into_parts();
      let version = format!("{:?}", &parts.version);
      let method = parts.method.as_str();
      let uri = format!("{}", &parts.uri);
      let headers = format!("{:?}", &parts.headers);
      let body_size_hint = body.size_hint().upper().map(|s| s as i64);
      let body = cx.env.create_external(body, body_size_hint)?;
      Ok(vec![
        cx.env.create_string(&version)?.into_unknown(),
        cx.env.create_string(method)?.into_unknown(),
        cx.env.create_string(&uri)?.into_unknown(),
        cx.env.create_string(&headers)?.into_unknown(),
        body.into_unknown(),
      ])
    },
  )?;

  let start = async move {
    let mut list = CALLBACKLIST.lock().await;
    list.push(req_tsfn_callback);
    Ok(())
  };

  ctx
    .env
    .execute_tokio_future(start, |env, _| env.get_undefined())
}
