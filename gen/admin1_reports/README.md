<!---
DO NOT EDIT !
This file was generated automatically from 'src/generator/templates/api/README.md.mako'
DO NOT EDIT !
-->
The `google-admin1_reports` library allows access to all features of the *Google reports* service.

This documentation was generated from *reports* crate version *5.0.3+20230117*, where *20230117* is the exact revision of the *admin:reports_v1* schema built by the [mako](http://www.makotemplates.org/) code generator *v5.0.3*.

Everything else about the *reports* *v1_reports* API can be found at the
[official documentation site](https://developers.google.com/admin-sdk/).
# Features

Handle the following *Resources* with ease from the central [hub](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/Reports) ... 

* [activities](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::Activity)
 * [*list*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::ActivityListCall) and [*watch*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::ActivityWatchCall)
* [channels](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::Channel)
 * [*stop*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::ChannelStopCall)
* customer usage reports
 * [*get*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::CustomerUsageReportGetCall)
* entity usage reports
 * [*get*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::EntityUsageReportGetCall)
* user usage report
 * [*get*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/api::UserUsageReportGetCall)




# Structure of this Library

The API is structured into the following primary items:

* **[Hub](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/Reports)**
    * a central object to maintain state and allow accessing all *Activities*
    * creates [*Method Builders*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::MethodsBuilder) which in turn
      allow access to individual [*Call Builders*](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::CallBuilder)
* **[Resources](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Resource)**
    * primary types that you can apply *Activities* to
    * a collection of properties and *Parts*
    * **[Parts](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Part)**
        * a collection of properties
        * never directly used in *Activities*
* **[Activities](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::CallBuilder)**
    * operations to apply to *Resources*

All *structures* are marked with applicable traits to further categorize them and ease browsing.

Generally speaking, you can invoke *Activities* like this:

```Rust,ignore
let r = hub.resource().activity(...).doit().await
```

Or specifically ...

```ignore
let r = hub.customer_usage_reports().get(...).doit().await
let r = hub.entity_usage_reports().get(...).doit().await
let r = hub.user_usage_report().get(...).doit().await
```

The `resource()` and `activity(...)` calls create [builders][builder-pattern]. The second one dealing with `Activities` 
supports various methods to configure the impending operation (not shown here). It is made such that all required arguments have to be 
specified right away (i.e. `(...)`), whereas all optional ones can be [build up][builder-pattern] as desired.
The `doit()` method performs the actual communication with the server and returns the respective result.

# Usage

## Setting up your Project

To use this library, you would put the following lines into your `Cargo.toml` file:

```toml
[dependencies]
google-admin1_reports = "*"
serde = "^1.0"
serde_json = "^1.0"
```

## A complete example

```Rust
extern crate hyper;
extern crate hyper_rustls;
extern crate google_admin1_reports as admin1_reports;
use admin1_reports::{Result, Error};
use std::default::Default;
use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};

// Get an ApplicationSecret instance by some means. It contains the `client_id` and 
// `client_secret`, among other things.
let secret: oauth2::ApplicationSecret = Default::default();
// Instantiate the authenticator. It will choose a suitable authentication flow for you, 
// unless you replace  `None` with the desired Flow.
// Provide your own `AuthenticatorDelegate` to adjust the way it operates and get feedback about 
// what's going on. You probably want to bring in your own `TokenStorage` to persist tokens and
// retrieve them from storage.
let auth = oauth2::InstalledFlowAuthenticator::builder(
        secret,
        oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    ).build().await.unwrap();
let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
// You can configure optional parameters by calling the respective setters at will, and
// execute the final call using `doit()`.
// Values shown here are possibly random and not representative !
let result = hub.user_usage_report().get("userKey", "date")
             .parameters("no")
             .page_token("ipsum")
             .org_unit_id("voluptua.")
             .max_results(74)
             .group_id_filter("sanctus")
             .filters("sed")
             .customer_id("amet.")
             .doit().await;

match result {
    Err(e) => match e {
        // The Error enum provides details about what exactly happened.
        // You can also just use its `Debug`, `Display` or `Error` traits
         Error::HttpError(_)
        |Error::Io(_)
        |Error::MissingAPIKey
        |Error::MissingToken(_)
        |Error::Cancelled
        |Error::UploadSizeLimitExceeded(_, _)
        |Error::Failure(_)
        |Error::BadRequest(_)
        |Error::FieldClash(_)
        |Error::JsonDecodeError(_, _) => println!("{}", e),
    },
    Ok(res) => println!("Success: {:?}", res),
}

```
## Handling Errors

All errors produced by the system are provided either as [Result](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Result) enumeration as return value of
the doit() methods, or handed as possibly intermediate results to either the 
[Hub Delegate](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Delegate), or the [Authenticator Delegate](https://docs.rs/yup-oauth2/*/yup_oauth2/trait.AuthenticatorDelegate.html).

When delegates handle errors or intermediate values, they may have a chance to instruct the system to retry. This 
makes the system potentially resilient to all kinds of errors.

## Uploads and Downloads
If a method supports downloads, the response body, which is part of the [Result](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Result), should be
read by you to obtain the media.
If such a method also supports a [Response Result](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::ResponseResult), it will return that by default.
You can see it as meta-data for the actual media. To trigger a media download, you will have to set up the builder by making
this call: `.param("alt", "media")`.

Methods supporting uploads can do so using up to 2 different protocols: 
*simple* and *resumable*. The distinctiveness of each is represented by customized 
`doit(...)` methods, which are then named `upload(...)` and `upload_resumable(...)` respectively.

## Customization and Callbacks

You may alter the way an `doit()` method is called by providing a [delegate](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Delegate) to the 
[Method Builder](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::CallBuilder) before making the final `doit()` call. 
Respective methods will be called to provide progress information, as well as determine whether the system should 
retry on failure.

The [delegate trait](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Delegate) is default-implemented, allowing you to customize it with minimal effort.

## Optional Parts in Server-Requests

All structures provided by this library are made to be [encodable](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::RequestValue) and 
[decodable](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::ResponseResult) via *json*. Optionals are used to indicate that partial requests are responses 
are valid.
Most optionals are are considered [Parts](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::Part) which are identifiable by name, which will be sent to 
the server to indicate either the set parts of the request or the desired parts in the response.

## Builder Arguments

Using [method builders](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::CallBuilder), you are able to prepare an action call by repeatedly calling it's methods.
These will always take a single argument, for which the following statements are true.

* [PODs][wiki-pod] are handed by copy
* strings are passed as `&str`
* [request values](https://docs.rs/google-admin1_reports/5.0.3+20230117/google_admin1_reports/client::RequestValue) are moved

Arguments will always be copied or cloned into the builder, to make them independent of their original life times.

[wiki-pod]: http://en.wikipedia.org/wiki/Plain_old_data_structure
[builder-pattern]: http://en.wikipedia.org/wiki/Builder_pattern
[google-go-api]: https://github.com/google/google-api-go-client

# License
The **admin1_reports** library was generated by Sebastian Thiel, and is placed 
under the *MIT* license.
You can read the full text at the repository's [license file][repo-license].

[repo-license]: https://github.com/Byron/google-apis-rsblob/main/LICENSE.md
