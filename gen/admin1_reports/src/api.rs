use std::collections::HashMap;
use std::cell::RefCell;
use std::default::Default;
use std::collections::BTreeSet;
use std::error::Error as StdError;
use serde_json as json;
use std::io;
use std::fs;
use std::mem;

use hyper::client::connect;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::sleep;
use tower_service;
use serde::{Serialize, Deserialize};

use crate::{client, client::GetToken, client::serde_with};

// ##############
// UTILITIES ###
// ############

/// Identifies the an OAuth2 authorization scope.
/// A scope is needed when requesting an
/// [authorization token](https://developers.google.com/youtube/v3/guides/authentication).
#[derive(PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Clone, Copy)]
pub enum Scope {
    /// View audit reports for your G Suite domain
    ReportAuditReadonly,

    /// View usage reports for your G Suite domain
    ReportUsageReadonly,
}

impl AsRef<str> for Scope {
    fn as_ref(&self) -> &str {
        match *self {
            Scope::ReportAuditReadonly => "https://www.googleapis.com/auth/admin.reports.audit.readonly",
            Scope::ReportUsageReadonly => "https://www.googleapis.com/auth/admin.reports.usage.readonly",
        }
    }
}

impl Default for Scope {
    fn default() -> Scope {
        Scope::ReportAuditReadonly
    }
}



// ########
// HUB ###
// ######

/// Central instance to access all Reports related resource activities
///
/// # Examples
///
/// Instantiate a new hub
///
/// ```test_harness,no_run
/// extern crate hyper;
/// extern crate hyper_rustls;
/// extern crate google_admin1_reports as admin1_reports;
/// use admin1_reports::{Result, Error};
/// # async fn dox() {
/// use std::default::Default;
/// use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// // Get an ApplicationSecret instance by some means. It contains the `client_id` and 
/// // `client_secret`, among other things.
/// let secret: oauth2::ApplicationSecret = Default::default();
/// // Instantiate the authenticator. It will choose a suitable authentication flow for you, 
/// // unless you replace  `None` with the desired Flow.
/// // Provide your own `AuthenticatorDelegate` to adjust the way it operates and get feedback about 
/// // what's going on. You probably want to bring in your own `TokenStorage` to persist tokens and
/// // retrieve them from storage.
/// let auth = oauth2::InstalledFlowAuthenticator::builder(
///         secret,
///         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
///     ).build().await.unwrap();
/// let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // You can configure optional parameters by calling the respective setters at will, and
/// // execute the final call using `doit()`.
/// // Values shown here are possibly random and not representative !
/// let result = hub.user_usage_report().get("userKey", "date")
///              .parameters("invidunt")
///              .page_token("amet")
///              .org_unit_id("duo")
///              .max_results(51)
///              .group_id_filter("sed")
///              .filters("ut")
///              .customer_id("gubergren")
///              .doit().await;
/// 
/// match result {
///     Err(e) => match e {
///         // The Error enum provides details about what exactly happened.
///         // You can also just use its `Debug`, `Display` or `Error` traits
///          Error::HttpError(_)
///         |Error::Io(_)
///         |Error::MissingAPIKey
///         |Error::MissingToken(_)
///         |Error::Cancelled
///         |Error::UploadSizeLimitExceeded(_, _)
///         |Error::Failure(_)
///         |Error::BadRequest(_)
///         |Error::FieldClash(_)
///         |Error::JsonDecodeError(_, _) => println!("{}", e),
///     },
///     Ok(res) => println!("Success: {:?}", res),
/// }
/// # }
/// ```
#[derive(Clone)]
pub struct Reports<S> {
    pub client: hyper::Client<S, hyper::body::Body>,
    pub auth: Box<dyn client::GetToken>,
    _user_agent: String,
    _base_url: String,
    _root_url: String,
}

impl<'a, S> client::Hub for Reports<S> {}

impl<'a, S> Reports<S> {

    pub fn new<A: 'static + client::GetToken>(client: hyper::Client<S, hyper::body::Body>, auth: A) -> Reports<S> {
        Reports {
            client,
            auth: Box::new(auth),
            _user_agent: "google-api-rust-client/5.0.3".to_string(),
            _base_url: "https://admin.googleapis.com/".to_string(),
            _root_url: "https://admin.googleapis.com/".to_string(),
        }
    }

    pub fn activities(&'a self) -> ActivityMethods<'a, S> {
        ActivityMethods { hub: &self }
    }
    pub fn channels(&'a self) -> ChannelMethods<'a, S> {
        ChannelMethods { hub: &self }
    }
    pub fn customer_usage_reports(&'a self) -> CustomerUsageReportMethods<'a, S> {
        CustomerUsageReportMethods { hub: &self }
    }
    pub fn entity_usage_reports(&'a self) -> EntityUsageReportMethods<'a, S> {
        EntityUsageReportMethods { hub: &self }
    }
    pub fn user_usage_report(&'a self) -> UserUsageReportMethods<'a, S> {
        UserUsageReportMethods { hub: &self }
    }

    /// Set the user-agent header field to use in all requests to the server.
    /// It defaults to `google-api-rust-client/5.0.3`.
    ///
    /// Returns the previously set user-agent.
    pub fn user_agent(&mut self, agent_name: String) -> String {
        mem::replace(&mut self._user_agent, agent_name)
    }

    /// Set the base url to use in all requests to the server.
    /// It defaults to `https://admin.googleapis.com/`.
    ///
    /// Returns the previously set base url.
    pub fn base_url(&mut self, new_base_url: String) -> String {
        mem::replace(&mut self._base_url, new_base_url)
    }

    /// Set the root url to use in all requests to the server.
    /// It defaults to `https://admin.googleapis.com/`.
    ///
    /// Returns the previously set root url.
    pub fn root_url(&mut self, new_root_url: String) -> String {
        mem::replace(&mut self._root_url, new_root_url)
    }
}


// ############
// SCHEMAS ###
// ##########
/// JSON template for a collection of activities.
/// 
/// # Activities
/// 
/// This type is used in activities, which are methods you may call on this type or where this type is involved in. 
/// The list links the activity name, along with information about where it is used (one of *request* and *response*).
/// 
/// * [list activities](ActivityListCall) (response)
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Activities {
    /// ETag of the resource.
    
    pub etag: Option<String>,
    /// Each activity record in the response.
    
    pub items: Option<Vec<Activity>>,
    /// The type of API resource. For an activity report, the value is `reports#activities`.
    
    pub kind: Option<String>,
    /// Token for retrieving the follow-on next page of the report. The `nextPageToken` value is used in the request's `pageToken` query string.
    #[serde(rename="nextPageToken")]
    
    pub next_page_token: Option<String>,
}

impl client::ResponseResult for Activities {}


/// JSON template for the activity resource.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Activity {
    /// User doing the action.
    
    pub actor: Option<ActivityActor>,
    /// ETag of the entry.
    
    pub etag: Option<String>,
    /// Activity events in the report.
    
    pub events: Option<Vec<ActivityEvents>>,
    /// Unique identifier for each activity record.
    
    pub id: Option<ActivityId>,
    /// IP address of the user doing the action. This is the Internet Protocol (IP) address of the user when logging into Google Workspace, which may or may not reflect the user's physical location. For example, the IP address can be the user's proxy server's address or a virtual private network (VPN) address. The API supports IPv4 and IPv6.
    #[serde(rename="ipAddress")]
    
    pub ip_address: Option<String>,
    /// The type of API resource. For an activity report, the value is `audit#activity`.
    
    pub kind: Option<String>,
    /// This is the domain that is affected by the report's event. For example domain of Admin console or the Drive application's document owner.
    #[serde(rename="ownerDomain")]
    
    pub owner_domain: Option<String>,
}

impl client::Part for Activity {}


/// A notification channel used to watch for resource changes.
/// 
/// # Activities
/// 
/// This type is used in activities, which are methods you may call on this type or where this type is involved in. 
/// The list links the activity name, along with information about where it is used (one of *request* and *response*).
/// 
/// * [watch activities](ActivityWatchCall) (request|response)
/// * [stop channels](ChannelStopCall) (request)
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    /// The address where notifications are delivered for this channel.
    
    pub address: Option<String>,
    /// Date and time of notification channel expiration, expressed as a Unix timestamp, in milliseconds. Optional.
    
    #[serde_as(as = "Option<::client::serde_with::DisplayFromStr>")]
    pub expiration: Option<i64>,
    /// A UUID or similar unique string that identifies this channel.
    
    pub id: Option<String>,
    /// Identifies this as a notification channel used to watch for changes to a resource, which is "`api#channel`".
    
    pub kind: Option<String>,
    /// Additional parameters controlling delivery channel behavior. Optional.
    
    pub params: Option<HashMap<String, String>>,
    /// A Boolean value to indicate whether payload is wanted. Optional.
    
    pub payload: Option<bool>,
    /// An opaque ID that identifies the resource being watched on this channel. Stable across different API versions.
    #[serde(rename="resourceId")]
    
    pub resource_id: Option<String>,
    /// A version-specific identifier for the watched resource.
    #[serde(rename="resourceUri")]
    
    pub resource_uri: Option<String>,
    /// An arbitrary string delivered to the target address with each notification delivered over this channel. Optional.
    
    pub token: Option<String>,
    /// The type of delivery mechanism used for this channel. The value should be set to `"web_hook"`.
    #[serde(rename="type")]
    
    pub type_: Option<String>,
}

impl client::RequestValue for Channel {}
impl client::Resource for Channel {}
impl client::ResponseResult for Channel {}


/// JSON template for a parameter used in various reports.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct NestedParameter {
    /// Boolean value of the parameter.
    #[serde(rename="boolValue")]
    
    pub bool_value: Option<bool>,
    /// Integer value of the parameter.
    #[serde(rename="intValue")]
    
    #[serde_as(as = "Option<::client::serde_with::DisplayFromStr>")]
    pub int_value: Option<i64>,
    /// Multiple boolean values of the parameter.
    #[serde(rename="multiBoolValue")]
    
    pub multi_bool_value: Option<Vec<bool>>,
    /// Multiple integer values of the parameter.
    #[serde(rename="multiIntValue")]
    
    #[serde_as(as = "Option<Vec<::client::serde_with::DisplayFromStr>>")]
    pub multi_int_value: Option<Vec<i64>>,
    /// Multiple string values of the parameter.
    #[serde(rename="multiValue")]
    
    pub multi_value: Option<Vec<String>>,
    /// The name of the parameter.
    
    pub name: Option<String>,
    /// String value of the parameter.
    
    pub value: Option<String>,
}

impl client::Part for NestedParameter {}


/// JSON template for a usage report.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct UsageReport {
    /// Output only. The date of the report request.
    
    pub date: Option<String>,
    /// Output only. Information about the type of the item.
    
    pub entity: Option<UsageReportEntity>,
    /// ETag of the resource.
    
    pub etag: Option<String>,
    /// The type of API resource. For a usage report, the value is `admin#reports#usageReport`.
    
    pub kind: Option<String>,
    /// Output only. Parameter value pairs for various applications. For the Entity Usage Report parameters and values, see [the Entity Usage parameters reference](https://developers.google.com/admin-sdk/reports/v1/reference/usage-ref-appendix-a/entities).
    
    pub parameters: Option<Vec<UsageReportParameters>>,
}

impl client::Part for UsageReport {}


/// There is no detailed description.
/// 
/// # Activities
/// 
/// This type is used in activities, which are methods you may call on this type or where this type is involved in. 
/// The list links the activity name, along with information about where it is used (one of *request* and *response*).
/// 
/// * [get customer usage reports](CustomerUsageReportGetCall) (response)
/// * [get entity usage reports](EntityUsageReportGetCall) (response)
/// * [get user usage report](UserUsageReportGetCall) (response)
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct UsageReports {
    /// ETag of the resource.
    
    pub etag: Option<String>,
    /// The type of API resource. For a usage report, the value is `admin#reports#usageReports`.
    
    pub kind: Option<String>,
    /// Token to specify next page. A report with multiple pages has a `nextPageToken` property in the response. For your follow-on requests getting all of the report's pages, enter the `nextPageToken` value in the `pageToken` query string.
    #[serde(rename="nextPageToken")]
    
    pub next_page_token: Option<String>,
    /// Various application parameter records.
    #[serde(rename="usageReports")]
    
    pub usage_reports: Option<Vec<UsageReport>>,
    /// Warnings, if any.
    
    pub warnings: Option<Vec<UsageReportsWarnings>>,
}

impl client::ResponseResult for UsageReports {}


/// User doing the action.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ActivityActor {
    /// The type of actor.
    #[serde(rename="callerType")]
    
    pub caller_type: Option<String>,
    /// The primary email address of the actor. May be absent if there is no email address associated with the actor.
    
    pub email: Option<String>,
    /// Only present when `callerType` is `KEY`. Can be the `consumer_key` of the requestor for OAuth 2LO API requests or an identifier for robot accounts.
    
    pub key: Option<String>,
    /// The unique Google Workspace profile ID of the actor. This value might be absent if the actor is not a Google Workspace user, or may be the number 105250506097979753968 which acts as a placeholder ID.
    #[serde(rename="profileId")]
    
    pub profile_id: Option<String>,
}

impl client::NestedType for ActivityActor {}
impl client::Part for ActivityActor {}


/// Activity events in the report.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEvents {
    /// Name of the event. This is the specific name of the activity reported by the API. And each `eventName` is related to a specific Google Workspace service or feature which the API organizes into types of events. For `eventName` request parameters in general: - If no `eventName` is given, the report returns all possible instances of an `eventName`. - When you request an `eventName`, the API's response returns all activities which contain that `eventName`. It is possible that the returned activities will have other `eventName` properties in addition to the one requested. For more information about `eventName` properties, see the list of event names for various applications above in `applicationName`.
    
    pub name: Option<String>,
    /// Parameter value pairs for various applications. For more information about `eventName` parameters, see the list of event names for various applications above in `applicationName`.
    
    pub parameters: Option<Vec<ActivityEventsParameters>>,
    /// Type of event. The Google Workspace service or feature that an administrator changes is identified in the `type` property which identifies an event using the `eventName` property. For a full list of the API's `type` categories, see the list of event names for various applications above in `applicationName`.
    #[serde(rename="type")]
    
    pub type_: Option<String>,
}

impl client::NestedType for ActivityEvents {}
impl client::Part for ActivityEvents {}


/// Parameter value pairs for various applications. For more information about `eventName` parameters, see the list of event names for various applications above in `applicationName`.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEventsParameters {
    /// Boolean value of the parameter.
    #[serde(rename="boolValue")]
    
    pub bool_value: Option<bool>,
    /// Integer value of the parameter.
    #[serde(rename="intValue")]
    
    #[serde_as(as = "Option<::client::serde_with::DisplayFromStr>")]
    pub int_value: Option<i64>,
    /// Nested parameter value pairs associated with this parameter. Complex value type for a parameter are returned as a list of parameter values. For example, the address parameter may have a value as `[{parameter: [{name: city, value: abc}]}]`
    #[serde(rename="messageValue")]
    
    pub message_value: Option<ActivityEventsParametersMessageValue>,
    /// Integer values of the parameter.
    #[serde(rename="multiIntValue")]
    
    #[serde_as(as = "Option<Vec<::client::serde_with::DisplayFromStr>>")]
    pub multi_int_value: Option<Vec<i64>>,
    /// List of `messageValue` objects.
    #[serde(rename="multiMessageValue")]
    
    pub multi_message_value: Option<Vec<ActivityEventsParametersMultiMessageValue>>,
    /// String values of the parameter.
    #[serde(rename="multiValue")]
    
    pub multi_value: Option<Vec<String>>,
    /// The name of the parameter.
    
    pub name: Option<String>,
    /// String value of the parameter.
    
    pub value: Option<String>,
}

impl client::NestedType for ActivityEventsParameters {}
impl client::Part for ActivityEventsParameters {}


/// Nested parameter value pairs associated with this parameter. Complex value type for a parameter are returned as a list of parameter values. For example, the address parameter may have a value as `[{parameter: [{name: city, value: abc}]}]`
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEventsParametersMessageValue {
    /// Parameter values
    
    pub parameter: Option<Vec<NestedParameter>>,
}

impl client::NestedType for ActivityEventsParametersMessageValue {}
impl client::Part for ActivityEventsParametersMessageValue {}


/// List of `messageValue` objects.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEventsParametersMultiMessageValue {
    /// Parameter values
    
    pub parameter: Option<Vec<NestedParameter>>,
}

impl client::NestedType for ActivityEventsParametersMultiMessageValue {}
impl client::Part for ActivityEventsParametersMultiMessageValue {}


/// Unique identifier for each activity record.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ActivityId {
    /// Application name to which the event belongs. For possible values see the list of applications above in `applicationName`.
    #[serde(rename="applicationName")]
    
    pub application_name: Option<String>,
    /// The unique identifier for a Google Workspace account.
    #[serde(rename="customerId")]
    
    pub customer_id: Option<String>,
    /// Time of occurrence of the activity. This is in UNIX epoch time in seconds.
    
    pub time: Option<client::chrono::DateTime<client::chrono::offset::Utc>>,
    /// Unique qualifier if multiple events have the same time.
    #[serde(rename="uniqueQualifier")]
    
    #[serde_as(as = "Option<::client::serde_with::DisplayFromStr>")]
    pub unique_qualifier: Option<i64>,
}

impl client::NestedType for ActivityId {}
impl client::Part for ActivityId {}


/// Output only. Information about the type of the item.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct UsageReportEntity {
    /// Output only. The unique identifier of the customer's account.
    #[serde(rename="customerId")]
    
    pub customer_id: Option<String>,
    /// Output only. Object key. Only relevant if entity.type = "OBJECT" Note: external-facing name of report is "Entities" rather than "Objects".
    #[serde(rename="entityId")]
    
    pub entity_id: Option<String>,
    /// Output only. The user's immutable Google Workspace profile identifier.
    #[serde(rename="profileId")]
    
    pub profile_id: Option<String>,
    /// Output only. The type of item. The value is `user`.
    #[serde(rename="type")]
    
    pub type_: Option<String>,
    /// Output only. The user's email address. Only relevant if entity.type = "USER"
    #[serde(rename="userEmail")]
    
    pub user_email: Option<String>,
}

impl client::NestedType for UsageReportEntity {}
impl client::Part for UsageReportEntity {}


/// Output only. Parameter value pairs for various applications. For the Entity Usage Report parameters and values, see [the Entity Usage parameters reference](https://developers.google.com/admin-sdk/reports/v1/reference/usage-ref-appendix-a/entities).
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct UsageReportParameters {
    /// Output only. Boolean value of the parameter.
    #[serde(rename="boolValue")]
    
    pub bool_value: Option<bool>,
    /// The RFC 3339 formatted value of the parameter, for example 2010-10-28T10:26:35.000Z.
    #[serde(rename="datetimeValue")]
    
    pub datetime_value: Option<client::chrono::DateTime<client::chrono::offset::Utc>>,
    /// Output only. Integer value of the parameter.
    #[serde(rename="intValue")]
    
    #[serde_as(as = "Option<::client::serde_with::DisplayFromStr>")]
    pub int_value: Option<i64>,
    /// Output only. Nested message value of the parameter.
    #[serde(rename="msgValue")]
    
    pub msg_value: Option<Vec<HashMap<String, json::Value>>>,
    /// The name of the parameter. For the User Usage Report parameter names, see the User Usage parameters reference.
    
    pub name: Option<String>,
    /// Output only. String value of the parameter.
    #[serde(rename="stringValue")]
    
    pub string_value: Option<String>,
}

impl client::NestedType for UsageReportParameters {}
impl client::Part for UsageReportParameters {}


/// Warnings, if any.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct UsageReportsWarnings {
    /// Machine readable code or warning type. The warning code value is `200`.
    
    pub code: Option<String>,
    /// Key-value pairs to give detailed information on the warning.
    
    pub data: Option<Vec<UsageReportsWarningsData>>,
    /// The human readable messages for a warning are: - Data is not available warning - Sorry, data for date yyyy-mm-dd for application "`application name`" is not available. - Partial data is available warning - Data for date yyyy-mm-dd for application "`application name`" is not available right now, please try again after a few hours. 
    
    pub message: Option<String>,
}

impl client::NestedType for UsageReportsWarnings {}
impl client::Part for UsageReportsWarnings {}


/// Key-value pairs to give detailed information on the warning.
/// 
/// This type is not used in any activity, and only used as *part* of another schema.
/// 
#[serde_with::serde_as(crate = "::client::serde_with")]
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct UsageReportsWarningsData {
    /// Key associated with a key-value pair to give detailed information on the warning.
    
    pub key: Option<String>,
    /// Value associated with a key-value pair to give detailed information on the warning.
    
    pub value: Option<String>,
}

impl client::NestedType for UsageReportsWarningsData {}
impl client::Part for UsageReportsWarningsData {}



// ###################
// MethodBuilders ###
// #################

/// A builder providing access to all methods supported on *activity* resources.
/// It is not used directly, but through the [`Reports`] hub.
///
/// # Example
///
/// Instantiate a resource builder
///
/// ```test_harness,no_run
/// extern crate hyper;
/// extern crate hyper_rustls;
/// extern crate google_admin1_reports as admin1_reports;
/// 
/// # async fn dox() {
/// use std::default::Default;
/// use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// let secret: oauth2::ApplicationSecret = Default::default();
/// let auth = oauth2::InstalledFlowAuthenticator::builder(
///         secret,
///         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
///     ).build().await.unwrap();
/// let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // Usually you wouldn't bind this to a variable, but keep calling *CallBuilders*
/// // like `list(...)` and `watch(...)`
/// // to build up your call.
/// let rb = hub.activities();
/// # }
/// ```
pub struct ActivityMethods<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
}

impl<'a, S> client::MethodsBuilder for ActivityMethods<'a, S> {}

impl<'a, S> ActivityMethods<'a, S> {
    
    /// Create a builder to help you perform the following task:
    ///
    /// Retrieves a list of activities for a specific customer's account and application such as the Admin console application or the Google Drive application. For more information, see the guides for administrator and Google Drive activity reports. For more information about the activity report's parameters, see the activity parameters reference guides. 
    /// 
    /// # Arguments
    ///
    /// * `userKey` - Represents the profile ID or the user email for which the data should be filtered. Can be `all` for all information, or `userKey` for a user's unique Google Workspace profile ID or their primary email address. Must not be a deleted user. For a deleted user, call `users.list` in Directory API with `showDeleted=true`, then use the returned `ID` as the `userKey`.
    /// * `applicationName` - Application name for which the events are to be retrieved.
    pub fn list(&self, user_key: &str, application_name: &str) -> ActivityListCall<'a, S> {
        ActivityListCall {
            hub: self.hub,
            _user_key: user_key.to_string(),
            _application_name: application_name.to_string(),
            _start_time: Default::default(),
            _page_token: Default::default(),
            _org_unit_id: Default::default(),
            _max_results: Default::default(),
            _group_id_filter: Default::default(),
            _filters: Default::default(),
            _event_name: Default::default(),
            _end_time: Default::default(),
            _customer_id: Default::default(),
            _actor_ip_address: Default::default(),
            _delegate: Default::default(),
            _additional_params: Default::default(),
            _scopes: Default::default(),
        }
    }
    
    /// Create a builder to help you perform the following task:
    ///
    /// Start receiving notifications for account activities. For more information, see Receiving Push Notifications.
    /// 
    /// # Arguments
    ///
    /// * `request` - No description provided.
    /// * `userKey` - Represents the profile ID or the user email for which the data should be filtered. Can be `all` for all information, or `userKey` for a user's unique Google Workspace profile ID or their primary email address. Must not be a deleted user. For a deleted user, call `users.list` in Directory API with `showDeleted=true`, then use the returned `ID` as the `userKey`.
    /// * `applicationName` - Application name for which the events are to be retrieved.
    pub fn watch(&self, request: Channel, user_key: &str, application_name: &str) -> ActivityWatchCall<'a, S> {
        ActivityWatchCall {
            hub: self.hub,
            _request: request,
            _user_key: user_key.to_string(),
            _application_name: application_name.to_string(),
            _start_time: Default::default(),
            _page_token: Default::default(),
            _org_unit_id: Default::default(),
            _max_results: Default::default(),
            _group_id_filter: Default::default(),
            _filters: Default::default(),
            _event_name: Default::default(),
            _end_time: Default::default(),
            _customer_id: Default::default(),
            _actor_ip_address: Default::default(),
            _delegate: Default::default(),
            _additional_params: Default::default(),
            _scopes: Default::default(),
        }
    }
}



/// A builder providing access to all methods supported on *channel* resources.
/// It is not used directly, but through the [`Reports`] hub.
///
/// # Example
///
/// Instantiate a resource builder
///
/// ```test_harness,no_run
/// extern crate hyper;
/// extern crate hyper_rustls;
/// extern crate google_admin1_reports as admin1_reports;
/// 
/// # async fn dox() {
/// use std::default::Default;
/// use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// let secret: oauth2::ApplicationSecret = Default::default();
/// let auth = oauth2::InstalledFlowAuthenticator::builder(
///         secret,
///         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
///     ).build().await.unwrap();
/// let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // Usually you wouldn't bind this to a variable, but keep calling *CallBuilders*
/// // like `stop(...)`
/// // to build up your call.
/// let rb = hub.channels();
/// # }
/// ```
pub struct ChannelMethods<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
}

impl<'a, S> client::MethodsBuilder for ChannelMethods<'a, S> {}

impl<'a, S> ChannelMethods<'a, S> {
    
    /// Create a builder to help you perform the following task:
    ///
    /// Stop watching resources through this channel.
    /// 
    /// # Arguments
    ///
    /// * `request` - No description provided.
    pub fn stop(&self, request: Channel) -> ChannelStopCall<'a, S> {
        ChannelStopCall {
            hub: self.hub,
            _request: request,
            _delegate: Default::default(),
            _additional_params: Default::default(),
            _scopes: Default::default(),
        }
    }
}



/// A builder providing access to all methods supported on *customerUsageReport* resources.
/// It is not used directly, but through the [`Reports`] hub.
///
/// # Example
///
/// Instantiate a resource builder
///
/// ```test_harness,no_run
/// extern crate hyper;
/// extern crate hyper_rustls;
/// extern crate google_admin1_reports as admin1_reports;
/// 
/// # async fn dox() {
/// use std::default::Default;
/// use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// let secret: oauth2::ApplicationSecret = Default::default();
/// let auth = oauth2::InstalledFlowAuthenticator::builder(
///         secret,
///         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
///     ).build().await.unwrap();
/// let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // Usually you wouldn't bind this to a variable, but keep calling *CallBuilders*
/// // like `get(...)`
/// // to build up your call.
/// let rb = hub.customer_usage_reports();
/// # }
/// ```
pub struct CustomerUsageReportMethods<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
}

impl<'a, S> client::MethodsBuilder for CustomerUsageReportMethods<'a, S> {}

impl<'a, S> CustomerUsageReportMethods<'a, S> {
    
    /// Create a builder to help you perform the following task:
    ///
    /// Retrieves a report which is a collection of properties and statistics for a specific customer's account. For more information, see the Customers Usage Report guide. For more information about the customer report's parameters, see the Customers Usage parameters reference guides. 
    /// 
    /// # Arguments
    ///
    /// * `date` - Represents the date the usage occurred, based on PST time zone. The timestamp is in the [ISO 8601 format](https://en.wikipedia.org/wiki/ISO_8601), `yyyy-mm-dd`.
    pub fn get(&self, date: &str) -> CustomerUsageReportGetCall<'a, S> {
        CustomerUsageReportGetCall {
            hub: self.hub,
            _date: date.to_string(),
            _parameters: Default::default(),
            _page_token: Default::default(),
            _customer_id: Default::default(),
            _delegate: Default::default(),
            _additional_params: Default::default(),
            _scopes: Default::default(),
        }
    }
}



/// A builder providing access to all methods supported on *entityUsageReport* resources.
/// It is not used directly, but through the [`Reports`] hub.
///
/// # Example
///
/// Instantiate a resource builder
///
/// ```test_harness,no_run
/// extern crate hyper;
/// extern crate hyper_rustls;
/// extern crate google_admin1_reports as admin1_reports;
/// 
/// # async fn dox() {
/// use std::default::Default;
/// use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// let secret: oauth2::ApplicationSecret = Default::default();
/// let auth = oauth2::InstalledFlowAuthenticator::builder(
///         secret,
///         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
///     ).build().await.unwrap();
/// let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // Usually you wouldn't bind this to a variable, but keep calling *CallBuilders*
/// // like `get(...)`
/// // to build up your call.
/// let rb = hub.entity_usage_reports();
/// # }
/// ```
pub struct EntityUsageReportMethods<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
}

impl<'a, S> client::MethodsBuilder for EntityUsageReportMethods<'a, S> {}

impl<'a, S> EntityUsageReportMethods<'a, S> {
    
    /// Create a builder to help you perform the following task:
    ///
    /// Retrieves a report which is a collection of properties and statistics for entities used by users within the account. For more information, see the Entities Usage Report guide. For more information about the entities report's parameters, see the Entities Usage parameters reference guides.
    /// 
    /// # Arguments
    ///
    /// * `entityType` - Represents the type of entity for the report.
    /// * `entityKey` - Represents the key of the object to filter the data with. It is a string which can take the value `all` to get activity events for all users, or any other value for an app-specific entity. For details on how to obtain the `entityKey` for a particular `entityType`, see the Entities Usage parameters reference guides.
    /// * `date` - Represents the date the usage occurred. The timestamp is in the ISO 8601 format, yyyy-mm-dd. We recommend you use your account's time zone for this.
    pub fn get(&self, entity_type: &str, entity_key: &str, date: &str) -> EntityUsageReportGetCall<'a, S> {
        EntityUsageReportGetCall {
            hub: self.hub,
            _entity_type: entity_type.to_string(),
            _entity_key: entity_key.to_string(),
            _date: date.to_string(),
            _parameters: Default::default(),
            _page_token: Default::default(),
            _max_results: Default::default(),
            _filters: Default::default(),
            _customer_id: Default::default(),
            _delegate: Default::default(),
            _additional_params: Default::default(),
            _scopes: Default::default(),
        }
    }
}



/// A builder providing access to all methods supported on *userUsageReport* resources.
/// It is not used directly, but through the [`Reports`] hub.
///
/// # Example
///
/// Instantiate a resource builder
///
/// ```test_harness,no_run
/// extern crate hyper;
/// extern crate hyper_rustls;
/// extern crate google_admin1_reports as admin1_reports;
/// 
/// # async fn dox() {
/// use std::default::Default;
/// use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// let secret: oauth2::ApplicationSecret = Default::default();
/// let auth = oauth2::InstalledFlowAuthenticator::builder(
///         secret,
///         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
///     ).build().await.unwrap();
/// let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // Usually you wouldn't bind this to a variable, but keep calling *CallBuilders*
/// // like `get(...)`
/// // to build up your call.
/// let rb = hub.user_usage_report();
/// # }
/// ```
pub struct UserUsageReportMethods<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
}

impl<'a, S> client::MethodsBuilder for UserUsageReportMethods<'a, S> {}

impl<'a, S> UserUsageReportMethods<'a, S> {
    
    /// Create a builder to help you perform the following task:
    ///
    /// Retrieves a report which is a collection of properties and statistics for a set of users with the account. For more information, see the User Usage Report guide. For more information about the user report's parameters, see the Users Usage parameters reference guides.
    /// 
    /// # Arguments
    ///
    /// * `userKey` - Represents the profile ID or the user email for which the data should be filtered. Can be `all` for all information, or `userKey` for a user's unique Google Workspace profile ID or their primary email address. Must not be a deleted user. For a deleted user, call `users.list` in Directory API with `showDeleted=true`, then use the returned `ID` as the `userKey`.
    /// * `date` - Represents the date the usage occurred, based on GMT-7:00 (Pacific Standard Time). The timestamp is in the [ISO 8601 format](https://en.wikipedia.org/wiki/ISO_8601), `yyyy-mm-dd`.
    pub fn get(&self, user_key: &str, date: &str) -> UserUsageReportGetCall<'a, S> {
        UserUsageReportGetCall {
            hub: self.hub,
            _user_key: user_key.to_string(),
            _date: date.to_string(),
            _parameters: Default::default(),
            _page_token: Default::default(),
            _org_unit_id: Default::default(),
            _max_results: Default::default(),
            _group_id_filter: Default::default(),
            _filters: Default::default(),
            _customer_id: Default::default(),
            _delegate: Default::default(),
            _additional_params: Default::default(),
            _scopes: Default::default(),
        }
    }
}





// ###################
// CallBuilders   ###
// #################

/// Retrieves a list of activities for a specific customer's account and application such as the Admin console application or the Google Drive application. For more information, see the guides for administrator and Google Drive activity reports. For more information about the activity report's parameters, see the activity parameters reference guides. 
///
/// A builder for the *list* method supported by a *activity* resource.
/// It is not used directly, but through a [`ActivityMethods`] instance.
///
/// # Example
///
/// Instantiate a resource method builder
///
/// ```test_harness,no_run
/// # extern crate hyper;
/// # extern crate hyper_rustls;
/// # extern crate google_admin1_reports as admin1_reports;
/// # async fn dox() {
/// # use std::default::Default;
/// # use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// # let secret: oauth2::ApplicationSecret = Default::default();
/// # let auth = oauth2::InstalledFlowAuthenticator::builder(
/// #         secret,
/// #         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
/// #     ).build().await.unwrap();
/// # let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // You can configure optional parameters by calling the respective setters at will, and
/// // execute the final call using `doit()`.
/// // Values shown here are possibly random and not representative !
/// let result = hub.activities().list("userKey", "applicationName")
///              .start_time("ipsum")
///              .page_token("ipsum")
///              .org_unit_id("est")
///              .max_results(-62)
///              .group_id_filter("ea")
///              .filters("dolor")
///              .event_name("Lorem")
///              .end_time("eos")
///              .customer_id("labore")
///              .actor_ip_address("sed")
///              .doit().await;
/// # }
/// ```
pub struct ActivityListCall<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
    _user_key: String,
    _application_name: String,
    _start_time: Option<String>,
    _page_token: Option<String>,
    _org_unit_id: Option<String>,
    _max_results: Option<i32>,
    _group_id_filter: Option<String>,
    _filters: Option<String>,
    _event_name: Option<String>,
    _end_time: Option<String>,
    _customer_id: Option<String>,
    _actor_ip_address: Option<String>,
    _delegate: Option<&'a mut dyn client::Delegate>,
    _additional_params: HashMap<String, String>,
    _scopes: BTreeSet<String>
}

impl<'a, S> client::CallBuilder for ActivityListCall<'a, S> {}

impl<'a, S> ActivityListCall<'a, S>
where
    S: tower_service::Service<http::Uri> + Clone + Send + Sync + 'static,
    S::Response: hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{


    /// Perform the operation you have build so far.
    pub async fn doit(mut self) -> client::Result<(hyper::Response<hyper::body::Body>, Activities)> {
        use std::io::{Read, Seek};
        use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH, AUTHORIZATION, USER_AGENT, LOCATION};
        use client::{ToParts, url::Params};
        use std::borrow::Cow;

        let mut dd = client::DefaultDelegate;
        let mut dlg: &mut dyn client::Delegate = self._delegate.unwrap_or(&mut dd);
        dlg.begin(client::MethodInfo { id: "reports.activities.list",
                               http_method: hyper::Method::GET });

        for &field in ["alt", "userKey", "applicationName", "startTime", "pageToken", "orgUnitID", "maxResults", "groupIdFilter", "filters", "eventName", "endTime", "customerId", "actorIpAddress"].iter() {
            if self._additional_params.contains_key(field) {
                dlg.finished(false);
                return Err(client::Error::FieldClash(field));
            }
        }

        let mut params = Params::with_capacity(14 + self._additional_params.len());
        params.push("userKey", self._user_key);
        params.push("applicationName", self._application_name);
        if let Some(value) = self._start_time.as_ref() {
            params.push("startTime", value);
        }
        if let Some(value) = self._page_token.as_ref() {
            params.push("pageToken", value);
        }
        if let Some(value) = self._org_unit_id.as_ref() {
            params.push("orgUnitID", value);
        }
        if let Some(value) = self._max_results.as_ref() {
            params.push("maxResults", value.to_string());
        }
        if let Some(value) = self._group_id_filter.as_ref() {
            params.push("groupIdFilter", value);
        }
        if let Some(value) = self._filters.as_ref() {
            params.push("filters", value);
        }
        if let Some(value) = self._event_name.as_ref() {
            params.push("eventName", value);
        }
        if let Some(value) = self._end_time.as_ref() {
            params.push("endTime", value);
        }
        if let Some(value) = self._customer_id.as_ref() {
            params.push("customerId", value);
        }
        if let Some(value) = self._actor_ip_address.as_ref() {
            params.push("actorIpAddress", value);
        }

        params.extend(self._additional_params.iter());

        params.push("alt", "json");
        let mut url = self.hub._base_url.clone() + "admin/reports/v1/activity/users/{userKey}/applications/{applicationName}";
        if self._scopes.is_empty() {
            self._scopes.insert(Scope::ReportAuditReadonly.as_ref().to_string());
        }

        for &(find_this, param_name) in [("{userKey}", "userKey"), ("{applicationName}", "applicationName")].iter() {
            url = params.uri_replacement(url, param_name, find_this, false);
        }
        {
            let to_remove = ["applicationName", "userKey"];
            params.remove_params(&to_remove);
        }

        let url = params.parse_with_url(&url);



        loop {
            let token = match self.hub.auth.get_token(&self._scopes.iter().map(String::as_str).collect::<Vec<_>>()[..]).await {
                Ok(token) => token,
                Err(e) => {
                    match dlg.token(e) {
                        Ok(token) => token,
                        Err(e) => {
                            dlg.finished(false);
                            return Err(client::Error::MissingToken(e));
                        }
                    }
                }
            };
            let mut req_result = {
                let client = &self.hub.client;
                dlg.pre_request();
                let mut req_builder = hyper::Request::builder()
                    .method(hyper::Method::GET)
                    .uri(url.as_str())
                    .header(USER_AGENT, self.hub._user_agent.clone());

                if let Some(token) = token.as_ref() {
                    req_builder = req_builder.header(AUTHORIZATION, format!("Bearer {}", token));
                }


                        let request = req_builder
                        .body(hyper::body::Body::empty());

                client.request(request.unwrap()).await

            };

            match req_result {
                Err(err) => {
                    if let client::Retry::After(d) = dlg.http_error(&err) {
                        sleep(d).await;
                        continue;
                    }
                    dlg.finished(false);
                    return Err(client::Error::HttpError(err))
                }
                Ok(mut res) => {
                    if !res.status().is_success() {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;
                        let (parts, _) = res.into_parts();
                        let body = hyper::Body::from(res_body_string.clone());
                        let restored_response = hyper::Response::from_parts(parts, body);

                        let server_response = json::from_str::<serde_json::Value>(&res_body_string).ok();

                        if let client::Retry::After(d) = dlg.http_failure(&restored_response, server_response.clone()) {
                            sleep(d).await;
                            continue;
                        }

                        dlg.finished(false);

                        return match server_response {
                            Some(error_value) => Err(client::Error::BadRequest(error_value)),
                            None => Err(client::Error::Failure(restored_response)),
                        }
                    }
                    let result_value = {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;

                        match json::from_str(&res_body_string) {
                            Ok(decoded) => (res, decoded),
                            Err(err) => {
                                dlg.response_json_decode_error(&res_body_string, &err);
                                return Err(client::Error::JsonDecodeError(res_body_string, err));
                            }
                        }
                    };

                    dlg.finished(true);
                    return Ok(result_value)
                }
            }
        }
    }


    /// Represents the profile ID or the user email for which the data should be filtered. Can be `all` for all information, or `userKey` for a user's unique Google Workspace profile ID or their primary email address. Must not be a deleted user. For a deleted user, call `users.list` in Directory API with `showDeleted=true`, then use the returned `ID` as the `userKey`.
    ///
    /// Sets the *user key* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn user_key(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._user_key = new_value.to_string();
        self
    }
    /// Application name for which the events are to be retrieved.
    ///
    /// Sets the *application name* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn application_name(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._application_name = new_value.to_string();
        self
    }
    /// Sets the beginning of the range of time shown in the report. The date is in the RFC 3339 format, for example 2010-10-28T10:26:35.000Z. The report returns all activities from `startTime` until `endTime`. The `startTime` must be before the `endTime` (if specified) and the current time when the request is made, or the API returns an error.
    ///
    /// Sets the *start time* query property to the given value.
    pub fn start_time(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._start_time = Some(new_value.to_string());
        self
    }
    /// The token to specify next page. A report with multiple pages has a `nextPageToken` property in the response. In your follow-on request getting the next page of the report, enter the `nextPageToken` value in the `pageToken` query string.
    ///
    /// Sets the *page token* query property to the given value.
    pub fn page_token(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._page_token = Some(new_value.to_string());
        self
    }
    /// ID of the organizational unit to report on. Activity records will be shown only for users who belong to the specified organizational unit. Data before Dec 17, 2018 doesn't appear in the filtered results.
    ///
    /// Sets the *org unit id* query property to the given value.
    pub fn org_unit_id(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._org_unit_id = Some(new_value.to_string());
        self
    }
    /// Determines how many activity records are shown on each response page. For example, if the request sets `maxResults=1` and the report has two activities, the report has two pages. The response's `nextPageToken` property has the token to the second page. The `maxResults` query string is optional in the request. The default value is 1000.
    ///
    /// Sets the *max results* query property to the given value.
    pub fn max_results(mut self, new_value: i32) -> ActivityListCall<'a, S> {
        self._max_results = Some(new_value);
        self
    }
    /// Comma separated group ids (obfuscated) on which user activities are filtered, i.e. the response will contain activities for only those users that are a part of at least one of the group ids mentioned here. Format: "id:abc123,id:xyz456"
    ///
    /// Sets the *group id filter* query property to the given value.
    pub fn group_id_filter(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._group_id_filter = Some(new_value.to_string());
        self
    }
    /// The `filters` query string is a comma-separated list composed of event parameters manipulated by relational operators. Event parameters are in the form `{parameter1 name}{relational operator}{parameter1 value},{parameter2 name}{relational operator}{parameter2 value},...` These event parameters are associated with a specific `eventName`. An empty report is returned if the request’s parameter doesn’t belong to the `eventName`. For more information about the available `eventName` fields for each application and their associated parameters, go to the [ApplicationName](#applicationname) table, then click through to the Activity Events page in the Appendix for the application you want. In the following Drive activity examples, the returned list consists of all `edit` events where the `doc_id` parameter value matches the conditions defined by the relational operator. In the first example, the request returns all edited documents with a `doc_id` value equal to `12345`. In the second example, the report returns any edited documents where the `doc_id` value is not equal to `98765`. The `<>` operator is URL-encoded in the request’s query string (`%3C%3E`): `GET...&eventName=edit&filters=doc_id==12345 GET...&eventName=edit&filters=doc_id%3C%3E98765` A `filters` query supports these relational operators: * `==`—‘equal to’. * `<>`—‘not equal to’. Must be URL-encoded (%3C%3E). * `<`—‘less than’. Must be URL-encoded (%3C). * `<=`—‘less than or equal to’. Must be URL-encoded (%3C=). * `>`—‘greater than’. Must be URL-encoded (%3E). * `>=`—‘greater than or equal to’. Must be URL-encoded (%3E=). **Note:** The API doesn’t accept multiple values of the same parameter. If a parameter is supplied more than once in the API request, the API only accepts the last value of that parameter. In addition, if an invalid parameter is supplied in the API request, the API ignores that parameter and returns the response corresponding to the remaining valid parameters. If no parameters are requested, all parameters are returned.
    ///
    /// Sets the *filters* query property to the given value.
    pub fn filters(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._filters = Some(new_value.to_string());
        self
    }
    /// The name of the event being queried by the API. Each `eventName` is related to a specific Google Workspace service or feature which the API organizes into types of events. An example is the Google Calendar events in the Admin console application's reports. The Calendar Settings `type` structure has all of the Calendar `eventName` activities reported by the API. When an administrator changes a Calendar setting, the API reports this activity in the Calendar Settings `type` and `eventName` parameters. For more information about `eventName` query strings and parameters, see the list of event names for various applications above in `applicationName`.
    ///
    /// Sets the *event name* query property to the given value.
    pub fn event_name(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._event_name = Some(new_value.to_string());
        self
    }
    /// Sets the end of the range of time shown in the report. The date is in the RFC 3339 format, for example 2010-10-28T10:26:35.000Z. The default value is the approximate time of the API request. An API report has three basic time concepts: - *Date of the API's request for a report*: When the API created and retrieved the report. - *Report's start time*: The beginning of the timespan shown in the report. The `startTime` must be before the `endTime` (if specified) and the current time when the request is made, or the API returns an error. - *Report's end time*: The end of the timespan shown in the report. For example, the timespan of events summarized in a report can start in April and end in May. The report itself can be requested in August. If the `endTime` is not specified, the report returns all activities from the `startTime` until the current time or the most recent 180 days if the `startTime` is more than 180 days in the past.
    ///
    /// Sets the *end time* query property to the given value.
    pub fn end_time(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._end_time = Some(new_value.to_string());
        self
    }
    /// The unique ID of the customer to retrieve data for.
    ///
    /// Sets the *customer id* query property to the given value.
    pub fn customer_id(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._customer_id = Some(new_value.to_string());
        self
    }
    /// The Internet Protocol (IP) Address of host where the event was performed. This is an additional way to filter a report's summary using the IP address of the user whose activity is being reported. This IP address may or may not reflect the user's physical location. For example, the IP address can be the user's proxy server's address or a virtual private network (VPN) address. This parameter supports both IPv4 and IPv6 address versions.
    ///
    /// Sets the *actor ip address* query property to the given value.
    pub fn actor_ip_address(mut self, new_value: &str) -> ActivityListCall<'a, S> {
        self._actor_ip_address = Some(new_value.to_string());
        self
    }
    /// The delegate implementation is consulted whenever there is an intermediate result, or if something goes wrong
    /// while executing the actual API request.
    /// 
    /// ````text
    ///                   It should be used to handle progress information, and to implement a certain level of resilience.
    /// ````
    ///
    /// Sets the *delegate* property to the given value.
    pub fn delegate(mut self, new_value: &'a mut dyn client::Delegate) -> ActivityListCall<'a, S> {
        self._delegate = Some(new_value);
        self
    }

    /// Set any additional parameter of the query string used in the request.
    /// It should be used to set parameters which are not yet available through their own
    /// setters.
    ///
    /// Please note that this method must not be used to set any of the known parameters
    /// which have their own setter method. If done anyway, the request will fail.
    ///
    /// # Additional Parameters
    ///
    /// * *$.xgafv* (query-string) - V1 error format.
    /// * *access_token* (query-string) - OAuth access token.
    /// * *alt* (query-string) - Data format for response.
    /// * *callback* (query-string) - JSONP
    /// * *fields* (query-string) - Selector specifying which fields to include in a partial response.
    /// * *key* (query-string) - API key. Your API key identifies your project and provides you with API access, quota, and reports. Required unless you provide an OAuth 2.0 token.
    /// * *oauth_token* (query-string) - OAuth 2.0 token for the current user.
    /// * *prettyPrint* (query-boolean) - Returns response with indentations and line breaks.
    /// * *quotaUser* (query-string) - Available to use for quota purposes for server-side applications. Can be any arbitrary string assigned to a user, but should not exceed 40 characters.
    /// * *uploadType* (query-string) - Legacy upload protocol for media (e.g. "media", "multipart").
    /// * *upload_protocol* (query-string) - Upload protocol for media (e.g. "raw", "multipart").
    pub fn param<T>(mut self, name: T, value: T) -> ActivityListCall<'a, S>
                                                        where T: AsRef<str> {
        self._additional_params.insert(name.as_ref().to_string(), value.as_ref().to_string());
        self
    }

    /// Identifies the authorization scope for the method you are building.
    ///
    /// Use this method to actively specify which scope should be used, instead of the default [`Scope`] variant
    /// [`Scope::ReportAuditReadonly`].
    ///
    /// The `scope` will be added to a set of scopes. This is important as one can maintain access
    /// tokens for more than one scope.
    ///
    /// Usually there is more than one suitable scope to authorize an operation, some of which may
    /// encompass more rights than others. For example, for listing resources, a *read-only* scope will be
    /// sufficient, a read-write scope will do as well.
    pub fn add_scope<St>(mut self, scope: St) -> ActivityListCall<'a, S>
                                                        where St: AsRef<str> {
        self._scopes.insert(String::from(scope.as_ref()));
        self
    }
    /// Identifies the authorization scope(s) for the method you are building.
    ///
    /// See [`Self::add_scope()`] for details.
    pub fn add_scopes<I, St>(mut self, scopes: I) -> ActivityListCall<'a, S>
                                                        where I: IntoIterator<Item = St>,
                                                         St: AsRef<str> {
        self._scopes
            .extend(scopes.into_iter().map(|s| String::from(s.as_ref())));
        self
    }

    /// Removes all scopes, and no default scope will be used either.
    /// In this case, you have to specify your API-key using the `key` parameter (see [`Self::param()`]
    /// for details).
    pub fn clear_scopes(mut self) -> ActivityListCall<'a, S> {
        self._scopes.clear();
        self
    }
}


/// Start receiving notifications for account activities. For more information, see Receiving Push Notifications.
///
/// A builder for the *watch* method supported by a *activity* resource.
/// It is not used directly, but through a [`ActivityMethods`] instance.
///
/// # Example
///
/// Instantiate a resource method builder
///
/// ```test_harness,no_run
/// # extern crate hyper;
/// # extern crate hyper_rustls;
/// # extern crate google_admin1_reports as admin1_reports;
/// use admin1_reports::api::Channel;
/// # async fn dox() {
/// # use std::default::Default;
/// # use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// # let secret: oauth2::ApplicationSecret = Default::default();
/// # let auth = oauth2::InstalledFlowAuthenticator::builder(
/// #         secret,
/// #         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
/// #     ).build().await.unwrap();
/// # let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // As the method needs a request, you would usually fill it with the desired information
/// // into the respective structure. Some of the parts shown here might not be applicable !
/// // Values shown here are possibly random and not representative !
/// let mut req = Channel::default();
/// 
/// // You can configure optional parameters by calling the respective setters at will, and
/// // execute the final call using `doit()`.
/// // Values shown here are possibly random and not representative !
/// let result = hub.activities().watch(req, "userKey", "applicationName")
///              .start_time("no")
///              .page_token("Stet")
///              .org_unit_id("kasd")
///              .max_results(-24)
///              .group_id_filter("sed")
///              .filters("et")
///              .event_name("et")
///              .end_time("vero")
///              .customer_id("erat")
///              .actor_ip_address("sed")
///              .doit().await;
/// # }
/// ```
pub struct ActivityWatchCall<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
    _request: Channel,
    _user_key: String,
    _application_name: String,
    _start_time: Option<String>,
    _page_token: Option<String>,
    _org_unit_id: Option<String>,
    _max_results: Option<i32>,
    _group_id_filter: Option<String>,
    _filters: Option<String>,
    _event_name: Option<String>,
    _end_time: Option<String>,
    _customer_id: Option<String>,
    _actor_ip_address: Option<String>,
    _delegate: Option<&'a mut dyn client::Delegate>,
    _additional_params: HashMap<String, String>,
    _scopes: BTreeSet<String>
}

impl<'a, S> client::CallBuilder for ActivityWatchCall<'a, S> {}

impl<'a, S> ActivityWatchCall<'a, S>
where
    S: tower_service::Service<http::Uri> + Clone + Send + Sync + 'static,
    S::Response: hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{


    /// Perform the operation you have build so far.
    pub async fn doit(mut self) -> client::Result<(hyper::Response<hyper::body::Body>, Channel)> {
        use std::io::{Read, Seek};
        use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH, AUTHORIZATION, USER_AGENT, LOCATION};
        use client::{ToParts, url::Params};
        use std::borrow::Cow;

        let mut dd = client::DefaultDelegate;
        let mut dlg: &mut dyn client::Delegate = self._delegate.unwrap_or(&mut dd);
        dlg.begin(client::MethodInfo { id: "reports.activities.watch",
                               http_method: hyper::Method::POST });

        for &field in ["alt", "userKey", "applicationName", "startTime", "pageToken", "orgUnitID", "maxResults", "groupIdFilter", "filters", "eventName", "endTime", "customerId", "actorIpAddress"].iter() {
            if self._additional_params.contains_key(field) {
                dlg.finished(false);
                return Err(client::Error::FieldClash(field));
            }
        }

        let mut params = Params::with_capacity(15 + self._additional_params.len());
        params.push("userKey", self._user_key);
        params.push("applicationName", self._application_name);
        if let Some(value) = self._start_time.as_ref() {
            params.push("startTime", value);
        }
        if let Some(value) = self._page_token.as_ref() {
            params.push("pageToken", value);
        }
        if let Some(value) = self._org_unit_id.as_ref() {
            params.push("orgUnitID", value);
        }
        if let Some(value) = self._max_results.as_ref() {
            params.push("maxResults", value.to_string());
        }
        if let Some(value) = self._group_id_filter.as_ref() {
            params.push("groupIdFilter", value);
        }
        if let Some(value) = self._filters.as_ref() {
            params.push("filters", value);
        }
        if let Some(value) = self._event_name.as_ref() {
            params.push("eventName", value);
        }
        if let Some(value) = self._end_time.as_ref() {
            params.push("endTime", value);
        }
        if let Some(value) = self._customer_id.as_ref() {
            params.push("customerId", value);
        }
        if let Some(value) = self._actor_ip_address.as_ref() {
            params.push("actorIpAddress", value);
        }

        params.extend(self._additional_params.iter());

        params.push("alt", "json");
        let mut url = self.hub._base_url.clone() + "admin/reports/v1/activity/users/{userKey}/applications/{applicationName}/watch";
        if self._scopes.is_empty() {
            self._scopes.insert(Scope::ReportAuditReadonly.as_ref().to_string());
        }

        for &(find_this, param_name) in [("{userKey}", "userKey"), ("{applicationName}", "applicationName")].iter() {
            url = params.uri_replacement(url, param_name, find_this, false);
        }
        {
            let to_remove = ["applicationName", "userKey"];
            params.remove_params(&to_remove);
        }

        let url = params.parse_with_url(&url);

        let mut json_mime_type = mime::APPLICATION_JSON;
        let mut request_value_reader =
            {
                let mut value = json::value::to_value(&self._request).expect("serde to work");
                client::remove_json_null_values(&mut value);
                let mut dst = io::Cursor::new(Vec::with_capacity(128));
                json::to_writer(&mut dst, &value).unwrap();
                dst
            };
        let request_size = request_value_reader.seek(io::SeekFrom::End(0)).unwrap();
        request_value_reader.seek(io::SeekFrom::Start(0)).unwrap();


        loop {
            let token = match self.hub.auth.get_token(&self._scopes.iter().map(String::as_str).collect::<Vec<_>>()[..]).await {
                Ok(token) => token,
                Err(e) => {
                    match dlg.token(e) {
                        Ok(token) => token,
                        Err(e) => {
                            dlg.finished(false);
                            return Err(client::Error::MissingToken(e));
                        }
                    }
                }
            };
            request_value_reader.seek(io::SeekFrom::Start(0)).unwrap();
            let mut req_result = {
                let client = &self.hub.client;
                dlg.pre_request();
                let mut req_builder = hyper::Request::builder()
                    .method(hyper::Method::POST)
                    .uri(url.as_str())
                    .header(USER_AGENT, self.hub._user_agent.clone());

                if let Some(token) = token.as_ref() {
                    req_builder = req_builder.header(AUTHORIZATION, format!("Bearer {}", token));
                }


                        let request = req_builder
                        .header(CONTENT_TYPE, json_mime_type.to_string())
                        .header(CONTENT_LENGTH, request_size as u64)
                        .body(hyper::body::Body::from(request_value_reader.get_ref().clone()));

                client.request(request.unwrap()).await

            };

            match req_result {
                Err(err) => {
                    if let client::Retry::After(d) = dlg.http_error(&err) {
                        sleep(d).await;
                        continue;
                    }
                    dlg.finished(false);
                    return Err(client::Error::HttpError(err))
                }
                Ok(mut res) => {
                    if !res.status().is_success() {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;
                        let (parts, _) = res.into_parts();
                        let body = hyper::Body::from(res_body_string.clone());
                        let restored_response = hyper::Response::from_parts(parts, body);

                        let server_response = json::from_str::<serde_json::Value>(&res_body_string).ok();

                        if let client::Retry::After(d) = dlg.http_failure(&restored_response, server_response.clone()) {
                            sleep(d).await;
                            continue;
                        }

                        dlg.finished(false);

                        return match server_response {
                            Some(error_value) => Err(client::Error::BadRequest(error_value)),
                            None => Err(client::Error::Failure(restored_response)),
                        }
                    }
                    let result_value = {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;

                        match json::from_str(&res_body_string) {
                            Ok(decoded) => (res, decoded),
                            Err(err) => {
                                dlg.response_json_decode_error(&res_body_string, &err);
                                return Err(client::Error::JsonDecodeError(res_body_string, err));
                            }
                        }
                    };

                    dlg.finished(true);
                    return Ok(result_value)
                }
            }
        }
    }


    ///
    /// Sets the *request* property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn request(mut self, new_value: Channel) -> ActivityWatchCall<'a, S> {
        self._request = new_value;
        self
    }
    /// Represents the profile ID or the user email for which the data should be filtered. Can be `all` for all information, or `userKey` for a user's unique Google Workspace profile ID or their primary email address. Must not be a deleted user. For a deleted user, call `users.list` in Directory API with `showDeleted=true`, then use the returned `ID` as the `userKey`.
    ///
    /// Sets the *user key* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn user_key(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._user_key = new_value.to_string();
        self
    }
    /// Application name for which the events are to be retrieved.
    ///
    /// Sets the *application name* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn application_name(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._application_name = new_value.to_string();
        self
    }
    /// Sets the beginning of the range of time shown in the report. The date is in the RFC 3339 format, for example 2010-10-28T10:26:35.000Z. The report returns all activities from `startTime` until `endTime`. The `startTime` must be before the `endTime` (if specified) and the current time when the request is made, or the API returns an error.
    ///
    /// Sets the *start time* query property to the given value.
    pub fn start_time(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._start_time = Some(new_value.to_string());
        self
    }
    /// The token to specify next page. A report with multiple pages has a `nextPageToken` property in the response. In your follow-on request getting the next page of the report, enter the `nextPageToken` value in the `pageToken` query string.
    ///
    /// Sets the *page token* query property to the given value.
    pub fn page_token(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._page_token = Some(new_value.to_string());
        self
    }
    /// ID of the organizational unit to report on. Activity records will be shown only for users who belong to the specified organizational unit. Data before Dec 17, 2018 doesn't appear in the filtered results.
    ///
    /// Sets the *org unit id* query property to the given value.
    pub fn org_unit_id(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._org_unit_id = Some(new_value.to_string());
        self
    }
    /// Determines how many activity records are shown on each response page. For example, if the request sets `maxResults=1` and the report has two activities, the report has two pages. The response's `nextPageToken` property has the token to the second page. The `maxResults` query string is optional in the request. The default value is 1000.
    ///
    /// Sets the *max results* query property to the given value.
    pub fn max_results(mut self, new_value: i32) -> ActivityWatchCall<'a, S> {
        self._max_results = Some(new_value);
        self
    }
    /// Comma separated group ids (obfuscated) on which user activities are filtered, i.e. the response will contain activities for only those users that are a part of at least one of the group ids mentioned here. Format: "id:abc123,id:xyz456"
    ///
    /// Sets the *group id filter* query property to the given value.
    pub fn group_id_filter(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._group_id_filter = Some(new_value.to_string());
        self
    }
    /// The `filters` query string is a comma-separated list composed of event parameters manipulated by relational operators. Event parameters are in the form `{parameter1 name}{relational operator}{parameter1 value},{parameter2 name}{relational operator}{parameter2 value},...` These event parameters are associated with a specific `eventName`. An empty report is returned if the request’s parameter doesn’t belong to the `eventName`. For more information about the available `eventName` fields for each application and their associated parameters, go to the [ApplicationName](#applicationname) table, then click through to the Activity Events page in the Appendix for the application you want. In the following Drive activity examples, the returned list consists of all `edit` events where the `doc_id` parameter value matches the conditions defined by the relational operator. In the first example, the request returns all edited documents with a `doc_id` value equal to `12345`. In the second example, the report returns any edited documents where the `doc_id` value is not equal to `98765`. The `<>` operator is URL-encoded in the request’s query string (`%3C%3E`): `GET...&eventName=edit&filters=doc_id==12345 GET...&eventName=edit&filters=doc_id%3C%3E98765` A `filters` query supports these relational operators: * `==`—‘equal to’. * `<>`—‘not equal to’. Must be URL-encoded (%3C%3E). * `<`—‘less than’. Must be URL-encoded (%3C). * `<=`—‘less than or equal to’. Must be URL-encoded (%3C=). * `>`—‘greater than’. Must be URL-encoded (%3E). * `>=`—‘greater than or equal to’. Must be URL-encoded (%3E=). **Note:** The API doesn’t accept multiple values of the same parameter. If a parameter is supplied more than once in the API request, the API only accepts the last value of that parameter. In addition, if an invalid parameter is supplied in the API request, the API ignores that parameter and returns the response corresponding to the remaining valid parameters. If no parameters are requested, all parameters are returned.
    ///
    /// Sets the *filters* query property to the given value.
    pub fn filters(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._filters = Some(new_value.to_string());
        self
    }
    /// The name of the event being queried by the API. Each `eventName` is related to a specific Google Workspace service or feature which the API organizes into types of events. An example is the Google Calendar events in the Admin console application's reports. The Calendar Settings `type` structure has all of the Calendar `eventName` activities reported by the API. When an administrator changes a Calendar setting, the API reports this activity in the Calendar Settings `type` and `eventName` parameters. For more information about `eventName` query strings and parameters, see the list of event names for various applications above in `applicationName`.
    ///
    /// Sets the *event name* query property to the given value.
    pub fn event_name(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._event_name = Some(new_value.to_string());
        self
    }
    /// Sets the end of the range of time shown in the report. The date is in the RFC 3339 format, for example 2010-10-28T10:26:35.000Z. The default value is the approximate time of the API request. An API report has three basic time concepts: - *Date of the API's request for a report*: When the API created and retrieved the report. - *Report's start time*: The beginning of the timespan shown in the report. The `startTime` must be before the `endTime` (if specified) and the current time when the request is made, or the API returns an error. - *Report's end time*: The end of the timespan shown in the report. For example, the timespan of events summarized in a report can start in April and end in May. The report itself can be requested in August. If the `endTime` is not specified, the report returns all activities from the `startTime` until the current time or the most recent 180 days if the `startTime` is more than 180 days in the past.
    ///
    /// Sets the *end time* query property to the given value.
    pub fn end_time(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._end_time = Some(new_value.to_string());
        self
    }
    /// The unique ID of the customer to retrieve data for.
    ///
    /// Sets the *customer id* query property to the given value.
    pub fn customer_id(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._customer_id = Some(new_value.to_string());
        self
    }
    /// The Internet Protocol (IP) Address of host where the event was performed. This is an additional way to filter a report's summary using the IP address of the user whose activity is being reported. This IP address may or may not reflect the user's physical location. For example, the IP address can be the user's proxy server's address or a virtual private network (VPN) address. This parameter supports both IPv4 and IPv6 address versions.
    ///
    /// Sets the *actor ip address* query property to the given value.
    pub fn actor_ip_address(mut self, new_value: &str) -> ActivityWatchCall<'a, S> {
        self._actor_ip_address = Some(new_value.to_string());
        self
    }
    /// The delegate implementation is consulted whenever there is an intermediate result, or if something goes wrong
    /// while executing the actual API request.
    /// 
    /// ````text
    ///                   It should be used to handle progress information, and to implement a certain level of resilience.
    /// ````
    ///
    /// Sets the *delegate* property to the given value.
    pub fn delegate(mut self, new_value: &'a mut dyn client::Delegate) -> ActivityWatchCall<'a, S> {
        self._delegate = Some(new_value);
        self
    }

    /// Set any additional parameter of the query string used in the request.
    /// It should be used to set parameters which are not yet available through their own
    /// setters.
    ///
    /// Please note that this method must not be used to set any of the known parameters
    /// which have their own setter method. If done anyway, the request will fail.
    ///
    /// # Additional Parameters
    ///
    /// * *$.xgafv* (query-string) - V1 error format.
    /// * *access_token* (query-string) - OAuth access token.
    /// * *alt* (query-string) - Data format for response.
    /// * *callback* (query-string) - JSONP
    /// * *fields* (query-string) - Selector specifying which fields to include in a partial response.
    /// * *key* (query-string) - API key. Your API key identifies your project and provides you with API access, quota, and reports. Required unless you provide an OAuth 2.0 token.
    /// * *oauth_token* (query-string) - OAuth 2.0 token for the current user.
    /// * *prettyPrint* (query-boolean) - Returns response with indentations and line breaks.
    /// * *quotaUser* (query-string) - Available to use for quota purposes for server-side applications. Can be any arbitrary string assigned to a user, but should not exceed 40 characters.
    /// * *uploadType* (query-string) - Legacy upload protocol for media (e.g. "media", "multipart").
    /// * *upload_protocol* (query-string) - Upload protocol for media (e.g. "raw", "multipart").
    pub fn param<T>(mut self, name: T, value: T) -> ActivityWatchCall<'a, S>
                                                        where T: AsRef<str> {
        self._additional_params.insert(name.as_ref().to_string(), value.as_ref().to_string());
        self
    }

    /// Identifies the authorization scope for the method you are building.
    ///
    /// Use this method to actively specify which scope should be used, instead of the default [`Scope`] variant
    /// [`Scope::ReportAuditReadonly`].
    ///
    /// The `scope` will be added to a set of scopes. This is important as one can maintain access
    /// tokens for more than one scope.
    ///
    /// Usually there is more than one suitable scope to authorize an operation, some of which may
    /// encompass more rights than others. For example, for listing resources, a *read-only* scope will be
    /// sufficient, a read-write scope will do as well.
    pub fn add_scope<St>(mut self, scope: St) -> ActivityWatchCall<'a, S>
                                                        where St: AsRef<str> {
        self._scopes.insert(String::from(scope.as_ref()));
        self
    }
    /// Identifies the authorization scope(s) for the method you are building.
    ///
    /// See [`Self::add_scope()`] for details.
    pub fn add_scopes<I, St>(mut self, scopes: I) -> ActivityWatchCall<'a, S>
                                                        where I: IntoIterator<Item = St>,
                                                         St: AsRef<str> {
        self._scopes
            .extend(scopes.into_iter().map(|s| String::from(s.as_ref())));
        self
    }

    /// Removes all scopes, and no default scope will be used either.
    /// In this case, you have to specify your API-key using the `key` parameter (see [`Self::param()`]
    /// for details).
    pub fn clear_scopes(mut self) -> ActivityWatchCall<'a, S> {
        self._scopes.clear();
        self
    }
}


/// Stop watching resources through this channel.
///
/// A builder for the *stop* method supported by a *channel* resource.
/// It is not used directly, but through a [`ChannelMethods`] instance.
///
/// # Example
///
/// Instantiate a resource method builder
///
/// ```test_harness,no_run
/// # extern crate hyper;
/// # extern crate hyper_rustls;
/// # extern crate google_admin1_reports as admin1_reports;
/// use admin1_reports::api::Channel;
/// # async fn dox() {
/// # use std::default::Default;
/// # use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// # let secret: oauth2::ApplicationSecret = Default::default();
/// # let auth = oauth2::InstalledFlowAuthenticator::builder(
/// #         secret,
/// #         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
/// #     ).build().await.unwrap();
/// # let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // As the method needs a request, you would usually fill it with the desired information
/// // into the respective structure. Some of the parts shown here might not be applicable !
/// // Values shown here are possibly random and not representative !
/// let mut req = Channel::default();
/// 
/// // You can configure optional parameters by calling the respective setters at will, and
/// // execute the final call using `doit()`.
/// // Values shown here are possibly random and not representative !
/// let result = hub.channels().stop(req)
///              .doit().await;
/// # }
/// ```
pub struct ChannelStopCall<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
    _request: Channel,
    _delegate: Option<&'a mut dyn client::Delegate>,
    _additional_params: HashMap<String, String>,
    _scopes: BTreeSet<String>
}

impl<'a, S> client::CallBuilder for ChannelStopCall<'a, S> {}

impl<'a, S> ChannelStopCall<'a, S>
where
    S: tower_service::Service<http::Uri> + Clone + Send + Sync + 'static,
    S::Response: hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{


    /// Perform the operation you have build so far.
    pub async fn doit(mut self) -> client::Result<hyper::Response<hyper::body::Body>> {
        use std::io::{Read, Seek};
        use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH, AUTHORIZATION, USER_AGENT, LOCATION};
        use client::{ToParts, url::Params};
        use std::borrow::Cow;

        let mut dd = client::DefaultDelegate;
        let mut dlg: &mut dyn client::Delegate = self._delegate.unwrap_or(&mut dd);
        dlg.begin(client::MethodInfo { id: "admin.channels.stop",
                               http_method: hyper::Method::POST });

        for &field in [].iter() {
            if self._additional_params.contains_key(field) {
                dlg.finished(false);
                return Err(client::Error::FieldClash(field));
            }
        }

        let mut params = Params::with_capacity(2 + self._additional_params.len());

        params.extend(self._additional_params.iter());

        let mut url = self.hub._base_url.clone() + "admin/reports_v1/channels/stop";
        if self._scopes.is_empty() {
            self._scopes.insert(Scope::ReportAuditReadonly.as_ref().to_string());
        }


        let url = params.parse_with_url(&url);

        let mut json_mime_type = mime::APPLICATION_JSON;
        let mut request_value_reader =
            {
                let mut value = json::value::to_value(&self._request).expect("serde to work");
                client::remove_json_null_values(&mut value);
                let mut dst = io::Cursor::new(Vec::with_capacity(128));
                json::to_writer(&mut dst, &value).unwrap();
                dst
            };
        let request_size = request_value_reader.seek(io::SeekFrom::End(0)).unwrap();
        request_value_reader.seek(io::SeekFrom::Start(0)).unwrap();


        loop {
            let token = match self.hub.auth.get_token(&self._scopes.iter().map(String::as_str).collect::<Vec<_>>()[..]).await {
                Ok(token) => token,
                Err(e) => {
                    match dlg.token(e) {
                        Ok(token) => token,
                        Err(e) => {
                            dlg.finished(false);
                            return Err(client::Error::MissingToken(e));
                        }
                    }
                }
            };
            request_value_reader.seek(io::SeekFrom::Start(0)).unwrap();
            let mut req_result = {
                let client = &self.hub.client;
                dlg.pre_request();
                let mut req_builder = hyper::Request::builder()
                    .method(hyper::Method::POST)
                    .uri(url.as_str())
                    .header(USER_AGENT, self.hub._user_agent.clone());

                if let Some(token) = token.as_ref() {
                    req_builder = req_builder.header(AUTHORIZATION, format!("Bearer {}", token));
                }


                        let request = req_builder
                        .header(CONTENT_TYPE, json_mime_type.to_string())
                        .header(CONTENT_LENGTH, request_size as u64)
                        .body(hyper::body::Body::from(request_value_reader.get_ref().clone()));

                client.request(request.unwrap()).await

            };

            match req_result {
                Err(err) => {
                    if let client::Retry::After(d) = dlg.http_error(&err) {
                        sleep(d).await;
                        continue;
                    }
                    dlg.finished(false);
                    return Err(client::Error::HttpError(err))
                }
                Ok(mut res) => {
                    if !res.status().is_success() {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;
                        let (parts, _) = res.into_parts();
                        let body = hyper::Body::from(res_body_string.clone());
                        let restored_response = hyper::Response::from_parts(parts, body);

                        let server_response = json::from_str::<serde_json::Value>(&res_body_string).ok();

                        if let client::Retry::After(d) = dlg.http_failure(&restored_response, server_response.clone()) {
                            sleep(d).await;
                            continue;
                        }

                        dlg.finished(false);

                        return match server_response {
                            Some(error_value) => Err(client::Error::BadRequest(error_value)),
                            None => Err(client::Error::Failure(restored_response)),
                        }
                    }
                    let result_value = res;

                    dlg.finished(true);
                    return Ok(result_value)
                }
            }
        }
    }


    ///
    /// Sets the *request* property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn request(mut self, new_value: Channel) -> ChannelStopCall<'a, S> {
        self._request = new_value;
        self
    }
    /// The delegate implementation is consulted whenever there is an intermediate result, or if something goes wrong
    /// while executing the actual API request.
    /// 
    /// ````text
    ///                   It should be used to handle progress information, and to implement a certain level of resilience.
    /// ````
    ///
    /// Sets the *delegate* property to the given value.
    pub fn delegate(mut self, new_value: &'a mut dyn client::Delegate) -> ChannelStopCall<'a, S> {
        self._delegate = Some(new_value);
        self
    }

    /// Set any additional parameter of the query string used in the request.
    /// It should be used to set parameters which are not yet available through their own
    /// setters.
    ///
    /// Please note that this method must not be used to set any of the known parameters
    /// which have their own setter method. If done anyway, the request will fail.
    ///
    /// # Additional Parameters
    ///
    /// * *$.xgafv* (query-string) - V1 error format.
    /// * *access_token* (query-string) - OAuth access token.
    /// * *alt* (query-string) - Data format for response.
    /// * *callback* (query-string) - JSONP
    /// * *fields* (query-string) - Selector specifying which fields to include in a partial response.
    /// * *key* (query-string) - API key. Your API key identifies your project and provides you with API access, quota, and reports. Required unless you provide an OAuth 2.0 token.
    /// * *oauth_token* (query-string) - OAuth 2.0 token for the current user.
    /// * *prettyPrint* (query-boolean) - Returns response with indentations and line breaks.
    /// * *quotaUser* (query-string) - Available to use for quota purposes for server-side applications. Can be any arbitrary string assigned to a user, but should not exceed 40 characters.
    /// * *uploadType* (query-string) - Legacy upload protocol for media (e.g. "media", "multipart").
    /// * *upload_protocol* (query-string) - Upload protocol for media (e.g. "raw", "multipart").
    pub fn param<T>(mut self, name: T, value: T) -> ChannelStopCall<'a, S>
                                                        where T: AsRef<str> {
        self._additional_params.insert(name.as_ref().to_string(), value.as_ref().to_string());
        self
    }

    /// Identifies the authorization scope for the method you are building.
    ///
    /// Use this method to actively specify which scope should be used, instead of the default [`Scope`] variant
    /// [`Scope::ReportAuditReadonly`].
    ///
    /// The `scope` will be added to a set of scopes. This is important as one can maintain access
    /// tokens for more than one scope.
    ///
    /// Usually there is more than one suitable scope to authorize an operation, some of which may
    /// encompass more rights than others. For example, for listing resources, a *read-only* scope will be
    /// sufficient, a read-write scope will do as well.
    pub fn add_scope<St>(mut self, scope: St) -> ChannelStopCall<'a, S>
                                                        where St: AsRef<str> {
        self._scopes.insert(String::from(scope.as_ref()));
        self
    }
    /// Identifies the authorization scope(s) for the method you are building.
    ///
    /// See [`Self::add_scope()`] for details.
    pub fn add_scopes<I, St>(mut self, scopes: I) -> ChannelStopCall<'a, S>
                                                        where I: IntoIterator<Item = St>,
                                                         St: AsRef<str> {
        self._scopes
            .extend(scopes.into_iter().map(|s| String::from(s.as_ref())));
        self
    }

    /// Removes all scopes, and no default scope will be used either.
    /// In this case, you have to specify your API-key using the `key` parameter (see [`Self::param()`]
    /// for details).
    pub fn clear_scopes(mut self) -> ChannelStopCall<'a, S> {
        self._scopes.clear();
        self
    }
}


/// Retrieves a report which is a collection of properties and statistics for a specific customer's account. For more information, see the Customers Usage Report guide. For more information about the customer report's parameters, see the Customers Usage parameters reference guides. 
///
/// A builder for the *get* method supported by a *customerUsageReport* resource.
/// It is not used directly, but through a [`CustomerUsageReportMethods`] instance.
///
/// # Example
///
/// Instantiate a resource method builder
///
/// ```test_harness,no_run
/// # extern crate hyper;
/// # extern crate hyper_rustls;
/// # extern crate google_admin1_reports as admin1_reports;
/// # async fn dox() {
/// # use std::default::Default;
/// # use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// # let secret: oauth2::ApplicationSecret = Default::default();
/// # let auth = oauth2::InstalledFlowAuthenticator::builder(
/// #         secret,
/// #         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
/// #     ).build().await.unwrap();
/// # let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // You can configure optional parameters by calling the respective setters at will, and
/// // execute the final call using `doit()`.
/// // Values shown here are possibly random and not representative !
/// let result = hub.customer_usage_reports().get("date")
///              .parameters("dolore")
///              .page_token("et")
///              .customer_id("voluptua.")
///              .doit().await;
/// # }
/// ```
pub struct CustomerUsageReportGetCall<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
    _date: String,
    _parameters: Option<String>,
    _page_token: Option<String>,
    _customer_id: Option<String>,
    _delegate: Option<&'a mut dyn client::Delegate>,
    _additional_params: HashMap<String, String>,
    _scopes: BTreeSet<String>
}

impl<'a, S> client::CallBuilder for CustomerUsageReportGetCall<'a, S> {}

impl<'a, S> CustomerUsageReportGetCall<'a, S>
where
    S: tower_service::Service<http::Uri> + Clone + Send + Sync + 'static,
    S::Response: hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{


    /// Perform the operation you have build so far.
    pub async fn doit(mut self) -> client::Result<(hyper::Response<hyper::body::Body>, UsageReports)> {
        use std::io::{Read, Seek};
        use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH, AUTHORIZATION, USER_AGENT, LOCATION};
        use client::{ToParts, url::Params};
        use std::borrow::Cow;

        let mut dd = client::DefaultDelegate;
        let mut dlg: &mut dyn client::Delegate = self._delegate.unwrap_or(&mut dd);
        dlg.begin(client::MethodInfo { id: "reports.customerUsageReports.get",
                               http_method: hyper::Method::GET });

        for &field in ["alt", "date", "parameters", "pageToken", "customerId"].iter() {
            if self._additional_params.contains_key(field) {
                dlg.finished(false);
                return Err(client::Error::FieldClash(field));
            }
        }

        let mut params = Params::with_capacity(6 + self._additional_params.len());
        params.push("date", self._date);
        if let Some(value) = self._parameters.as_ref() {
            params.push("parameters", value);
        }
        if let Some(value) = self._page_token.as_ref() {
            params.push("pageToken", value);
        }
        if let Some(value) = self._customer_id.as_ref() {
            params.push("customerId", value);
        }

        params.extend(self._additional_params.iter());

        params.push("alt", "json");
        let mut url = self.hub._base_url.clone() + "admin/reports/v1/usage/dates/{date}";
        if self._scopes.is_empty() {
            self._scopes.insert(Scope::ReportUsageReadonly.as_ref().to_string());
        }

        for &(find_this, param_name) in [("{date}", "date")].iter() {
            url = params.uri_replacement(url, param_name, find_this, false);
        }
        {
            let to_remove = ["date"];
            params.remove_params(&to_remove);
        }

        let url = params.parse_with_url(&url);



        loop {
            let token = match self.hub.auth.get_token(&self._scopes.iter().map(String::as_str).collect::<Vec<_>>()[..]).await {
                Ok(token) => token,
                Err(e) => {
                    match dlg.token(e) {
                        Ok(token) => token,
                        Err(e) => {
                            dlg.finished(false);
                            return Err(client::Error::MissingToken(e));
                        }
                    }
                }
            };
            let mut req_result = {
                let client = &self.hub.client;
                dlg.pre_request();
                let mut req_builder = hyper::Request::builder()
                    .method(hyper::Method::GET)
                    .uri(url.as_str())
                    .header(USER_AGENT, self.hub._user_agent.clone());

                if let Some(token) = token.as_ref() {
                    req_builder = req_builder.header(AUTHORIZATION, format!("Bearer {}", token));
                }


                        let request = req_builder
                        .body(hyper::body::Body::empty());

                client.request(request.unwrap()).await

            };

            match req_result {
                Err(err) => {
                    if let client::Retry::After(d) = dlg.http_error(&err) {
                        sleep(d).await;
                        continue;
                    }
                    dlg.finished(false);
                    return Err(client::Error::HttpError(err))
                }
                Ok(mut res) => {
                    if !res.status().is_success() {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;
                        let (parts, _) = res.into_parts();
                        let body = hyper::Body::from(res_body_string.clone());
                        let restored_response = hyper::Response::from_parts(parts, body);

                        let server_response = json::from_str::<serde_json::Value>(&res_body_string).ok();

                        if let client::Retry::After(d) = dlg.http_failure(&restored_response, server_response.clone()) {
                            sleep(d).await;
                            continue;
                        }

                        dlg.finished(false);

                        return match server_response {
                            Some(error_value) => Err(client::Error::BadRequest(error_value)),
                            None => Err(client::Error::Failure(restored_response)),
                        }
                    }
                    let result_value = {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;

                        match json::from_str(&res_body_string) {
                            Ok(decoded) => (res, decoded),
                            Err(err) => {
                                dlg.response_json_decode_error(&res_body_string, &err);
                                return Err(client::Error::JsonDecodeError(res_body_string, err));
                            }
                        }
                    };

                    dlg.finished(true);
                    return Ok(result_value)
                }
            }
        }
    }


    /// Represents the date the usage occurred, based on PST time zone. The timestamp is in the [ISO 8601 format](https://en.wikipedia.org/wiki/ISO_8601), `yyyy-mm-dd`.
    ///
    /// Sets the *date* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn date(mut self, new_value: &str) -> CustomerUsageReportGetCall<'a, S> {
        self._date = new_value.to_string();
        self
    }
    /// The `parameters` query string is a comma-separated list of event parameters that refine a report's results. The parameter is associated with a specific application. The application values for the Customers usage report include `accounts`, `app_maker`, `apps_scripts`, `calendar`, `classroom`, `cros`, `docs`, `gmail`, `gplus`, `device_management`, `meet`, and `sites`. A `parameters` query string is in the CSV form of `app_name1:param_name1, app_name2:param_name2`. *Note:* The API doesn't accept multiple values of a parameter. If a particular parameter is supplied more than once in the API request, the API only accepts the last value of that request parameter. In addition, if an invalid request parameter is supplied in the API request, the API ignores that request parameter and returns the response corresponding to the remaining valid request parameters. An example of an invalid request parameter is one that does not belong to the application. If no parameters are requested, all parameters are returned. 
    ///
    /// Sets the *parameters* query property to the given value.
    pub fn parameters(mut self, new_value: &str) -> CustomerUsageReportGetCall<'a, S> {
        self._parameters = Some(new_value.to_string());
        self
    }
    /// Token to specify next page. A report with multiple pages has a `nextPageToken` property in the response. For your follow-on requests getting all of the report's pages, enter the `nextPageToken` value in the `pageToken` query string.
    ///
    /// Sets the *page token* query property to the given value.
    pub fn page_token(mut self, new_value: &str) -> CustomerUsageReportGetCall<'a, S> {
        self._page_token = Some(new_value.to_string());
        self
    }
    /// The unique ID of the customer to retrieve data for.
    ///
    /// Sets the *customer id* query property to the given value.
    pub fn customer_id(mut self, new_value: &str) -> CustomerUsageReportGetCall<'a, S> {
        self._customer_id = Some(new_value.to_string());
        self
    }
    /// The delegate implementation is consulted whenever there is an intermediate result, or if something goes wrong
    /// while executing the actual API request.
    /// 
    /// ````text
    ///                   It should be used to handle progress information, and to implement a certain level of resilience.
    /// ````
    ///
    /// Sets the *delegate* property to the given value.
    pub fn delegate(mut self, new_value: &'a mut dyn client::Delegate) -> CustomerUsageReportGetCall<'a, S> {
        self._delegate = Some(new_value);
        self
    }

    /// Set any additional parameter of the query string used in the request.
    /// It should be used to set parameters which are not yet available through their own
    /// setters.
    ///
    /// Please note that this method must not be used to set any of the known parameters
    /// which have their own setter method. If done anyway, the request will fail.
    ///
    /// # Additional Parameters
    ///
    /// * *$.xgafv* (query-string) - V1 error format.
    /// * *access_token* (query-string) - OAuth access token.
    /// * *alt* (query-string) - Data format for response.
    /// * *callback* (query-string) - JSONP
    /// * *fields* (query-string) - Selector specifying which fields to include in a partial response.
    /// * *key* (query-string) - API key. Your API key identifies your project and provides you with API access, quota, and reports. Required unless you provide an OAuth 2.0 token.
    /// * *oauth_token* (query-string) - OAuth 2.0 token for the current user.
    /// * *prettyPrint* (query-boolean) - Returns response with indentations and line breaks.
    /// * *quotaUser* (query-string) - Available to use for quota purposes for server-side applications. Can be any arbitrary string assigned to a user, but should not exceed 40 characters.
    /// * *uploadType* (query-string) - Legacy upload protocol for media (e.g. "media", "multipart").
    /// * *upload_protocol* (query-string) - Upload protocol for media (e.g. "raw", "multipart").
    pub fn param<T>(mut self, name: T, value: T) -> CustomerUsageReportGetCall<'a, S>
                                                        where T: AsRef<str> {
        self._additional_params.insert(name.as_ref().to_string(), value.as_ref().to_string());
        self
    }

    /// Identifies the authorization scope for the method you are building.
    ///
    /// Use this method to actively specify which scope should be used, instead of the default [`Scope`] variant
    /// [`Scope::ReportUsageReadonly`].
    ///
    /// The `scope` will be added to a set of scopes. This is important as one can maintain access
    /// tokens for more than one scope.
    ///
    /// Usually there is more than one suitable scope to authorize an operation, some of which may
    /// encompass more rights than others. For example, for listing resources, a *read-only* scope will be
    /// sufficient, a read-write scope will do as well.
    pub fn add_scope<St>(mut self, scope: St) -> CustomerUsageReportGetCall<'a, S>
                                                        where St: AsRef<str> {
        self._scopes.insert(String::from(scope.as_ref()));
        self
    }
    /// Identifies the authorization scope(s) for the method you are building.
    ///
    /// See [`Self::add_scope()`] for details.
    pub fn add_scopes<I, St>(mut self, scopes: I) -> CustomerUsageReportGetCall<'a, S>
                                                        where I: IntoIterator<Item = St>,
                                                         St: AsRef<str> {
        self._scopes
            .extend(scopes.into_iter().map(|s| String::from(s.as_ref())));
        self
    }

    /// Removes all scopes, and no default scope will be used either.
    /// In this case, you have to specify your API-key using the `key` parameter (see [`Self::param()`]
    /// for details).
    pub fn clear_scopes(mut self) -> CustomerUsageReportGetCall<'a, S> {
        self._scopes.clear();
        self
    }
}


/// Retrieves a report which is a collection of properties and statistics for entities used by users within the account. For more information, see the Entities Usage Report guide. For more information about the entities report's parameters, see the Entities Usage parameters reference guides.
///
/// A builder for the *get* method supported by a *entityUsageReport* resource.
/// It is not used directly, but through a [`EntityUsageReportMethods`] instance.
///
/// # Example
///
/// Instantiate a resource method builder
///
/// ```test_harness,no_run
/// # extern crate hyper;
/// # extern crate hyper_rustls;
/// # extern crate google_admin1_reports as admin1_reports;
/// # async fn dox() {
/// # use std::default::Default;
/// # use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// # let secret: oauth2::ApplicationSecret = Default::default();
/// # let auth = oauth2::InstalledFlowAuthenticator::builder(
/// #         secret,
/// #         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
/// #     ).build().await.unwrap();
/// # let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // You can configure optional parameters by calling the respective setters at will, and
/// // execute the final call using `doit()`.
/// // Values shown here are possibly random and not representative !
/// let result = hub.entity_usage_reports().get("entityType", "entityKey", "date")
///              .parameters("dolor")
///              .page_token("et")
///              .max_results(79)
///              .filters("sadipscing")
///              .customer_id("Stet")
///              .doit().await;
/// # }
/// ```
pub struct EntityUsageReportGetCall<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
    _entity_type: String,
    _entity_key: String,
    _date: String,
    _parameters: Option<String>,
    _page_token: Option<String>,
    _max_results: Option<u32>,
    _filters: Option<String>,
    _customer_id: Option<String>,
    _delegate: Option<&'a mut dyn client::Delegate>,
    _additional_params: HashMap<String, String>,
    _scopes: BTreeSet<String>
}

impl<'a, S> client::CallBuilder for EntityUsageReportGetCall<'a, S> {}

impl<'a, S> EntityUsageReportGetCall<'a, S>
where
    S: tower_service::Service<http::Uri> + Clone + Send + Sync + 'static,
    S::Response: hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{


    /// Perform the operation you have build so far.
    pub async fn doit(mut self) -> client::Result<(hyper::Response<hyper::body::Body>, UsageReports)> {
        use std::io::{Read, Seek};
        use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH, AUTHORIZATION, USER_AGENT, LOCATION};
        use client::{ToParts, url::Params};
        use std::borrow::Cow;

        let mut dd = client::DefaultDelegate;
        let mut dlg: &mut dyn client::Delegate = self._delegate.unwrap_or(&mut dd);
        dlg.begin(client::MethodInfo { id: "reports.entityUsageReports.get",
                               http_method: hyper::Method::GET });

        for &field in ["alt", "entityType", "entityKey", "date", "parameters", "pageToken", "maxResults", "filters", "customerId"].iter() {
            if self._additional_params.contains_key(field) {
                dlg.finished(false);
                return Err(client::Error::FieldClash(field));
            }
        }

        let mut params = Params::with_capacity(10 + self._additional_params.len());
        params.push("entityType", self._entity_type);
        params.push("entityKey", self._entity_key);
        params.push("date", self._date);
        if let Some(value) = self._parameters.as_ref() {
            params.push("parameters", value);
        }
        if let Some(value) = self._page_token.as_ref() {
            params.push("pageToken", value);
        }
        if let Some(value) = self._max_results.as_ref() {
            params.push("maxResults", value.to_string());
        }
        if let Some(value) = self._filters.as_ref() {
            params.push("filters", value);
        }
        if let Some(value) = self._customer_id.as_ref() {
            params.push("customerId", value);
        }

        params.extend(self._additional_params.iter());

        params.push("alt", "json");
        let mut url = self.hub._base_url.clone() + "admin/reports/v1/usage/{entityType}/{entityKey}/dates/{date}";
        if self._scopes.is_empty() {
            self._scopes.insert(Scope::ReportUsageReadonly.as_ref().to_string());
        }

        for &(find_this, param_name) in [("{entityType}", "entityType"), ("{entityKey}", "entityKey"), ("{date}", "date")].iter() {
            url = params.uri_replacement(url, param_name, find_this, false);
        }
        {
            let to_remove = ["date", "entityKey", "entityType"];
            params.remove_params(&to_remove);
        }

        let url = params.parse_with_url(&url);



        loop {
            let token = match self.hub.auth.get_token(&self._scopes.iter().map(String::as_str).collect::<Vec<_>>()[..]).await {
                Ok(token) => token,
                Err(e) => {
                    match dlg.token(e) {
                        Ok(token) => token,
                        Err(e) => {
                            dlg.finished(false);
                            return Err(client::Error::MissingToken(e));
                        }
                    }
                }
            };
            let mut req_result = {
                let client = &self.hub.client;
                dlg.pre_request();
                let mut req_builder = hyper::Request::builder()
                    .method(hyper::Method::GET)
                    .uri(url.as_str())
                    .header(USER_AGENT, self.hub._user_agent.clone());

                if let Some(token) = token.as_ref() {
                    req_builder = req_builder.header(AUTHORIZATION, format!("Bearer {}", token));
                }


                        let request = req_builder
                        .body(hyper::body::Body::empty());

                client.request(request.unwrap()).await

            };

            match req_result {
                Err(err) => {
                    if let client::Retry::After(d) = dlg.http_error(&err) {
                        sleep(d).await;
                        continue;
                    }
                    dlg.finished(false);
                    return Err(client::Error::HttpError(err))
                }
                Ok(mut res) => {
                    if !res.status().is_success() {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;
                        let (parts, _) = res.into_parts();
                        let body = hyper::Body::from(res_body_string.clone());
                        let restored_response = hyper::Response::from_parts(parts, body);

                        let server_response = json::from_str::<serde_json::Value>(&res_body_string).ok();

                        if let client::Retry::After(d) = dlg.http_failure(&restored_response, server_response.clone()) {
                            sleep(d).await;
                            continue;
                        }

                        dlg.finished(false);

                        return match server_response {
                            Some(error_value) => Err(client::Error::BadRequest(error_value)),
                            None => Err(client::Error::Failure(restored_response)),
                        }
                    }
                    let result_value = {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;

                        match json::from_str(&res_body_string) {
                            Ok(decoded) => (res, decoded),
                            Err(err) => {
                                dlg.response_json_decode_error(&res_body_string, &err);
                                return Err(client::Error::JsonDecodeError(res_body_string, err));
                            }
                        }
                    };

                    dlg.finished(true);
                    return Ok(result_value)
                }
            }
        }
    }


    /// Represents the type of entity for the report.
    ///
    /// Sets the *entity type* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn entity_type(mut self, new_value: &str) -> EntityUsageReportGetCall<'a, S> {
        self._entity_type = new_value.to_string();
        self
    }
    /// Represents the key of the object to filter the data with. It is a string which can take the value `all` to get activity events for all users, or any other value for an app-specific entity. For details on how to obtain the `entityKey` for a particular `entityType`, see the Entities Usage parameters reference guides.
    ///
    /// Sets the *entity key* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn entity_key(mut self, new_value: &str) -> EntityUsageReportGetCall<'a, S> {
        self._entity_key = new_value.to_string();
        self
    }
    /// Represents the date the usage occurred. The timestamp is in the ISO 8601 format, yyyy-mm-dd. We recommend you use your account's time zone for this.
    ///
    /// Sets the *date* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn date(mut self, new_value: &str) -> EntityUsageReportGetCall<'a, S> {
        self._date = new_value.to_string();
        self
    }
    /// The `parameters` query string is a comma-separated list of event parameters that refine a report's results. The parameter is associated with a specific application. The application values for the Entities usage report are only `gplus`. A `parameter` query string is in the CSV form of `[app_name1:param_name1], [app_name2:param_name2]...`. *Note:* The API doesn't accept multiple values of a parameter. If a particular parameter is supplied more than once in the API request, the API only accepts the last value of that request parameter. In addition, if an invalid request parameter is supplied in the API request, the API ignores that request parameter and returns the response corresponding to the remaining valid request parameters. An example of an invalid request parameter is one that does not belong to the application. If no parameters are requested, all parameters are returned. 
    ///
    /// Sets the *parameters* query property to the given value.
    pub fn parameters(mut self, new_value: &str) -> EntityUsageReportGetCall<'a, S> {
        self._parameters = Some(new_value.to_string());
        self
    }
    /// Token to specify next page. A report with multiple pages has a `nextPageToken` property in the response. In your follow-on request getting the next page of the report, enter the `nextPageToken` value in the `pageToken` query string.
    ///
    /// Sets the *page token* query property to the given value.
    pub fn page_token(mut self, new_value: &str) -> EntityUsageReportGetCall<'a, S> {
        self._page_token = Some(new_value.to_string());
        self
    }
    /// Determines how many activity records are shown on each response page. For example, if the request sets `maxResults=1` and the report has two activities, the report has two pages. The response's `nextPageToken` property has the token to the second page.
    ///
    /// Sets the *max results* query property to the given value.
    pub fn max_results(mut self, new_value: u32) -> EntityUsageReportGetCall<'a, S> {
        self._max_results = Some(new_value);
        self
    }
    /// The `filters` query string is a comma-separated list of an application's event parameters where the parameter's value is manipulated by a relational operator. The `filters` query string includes the name of the application whose usage is returned in the report. The application values for the Entities usage report include `accounts`, `docs`, and `gmail`. Filters are in the form `[application name]:parameter name[parameter value],...`. In this example, the `<>` 'not equal to' operator is URL-encoded in the request's query string (%3C%3E): GET https://www.googleapis.com/admin/reports/v1/usage/gplus_communities/all/dates/2017-12-01 ?parameters=gplus:community_name,gplus:num_total_members &filters=gplus:num_total_members%3C%3E0 The relational operators include: - `==` - 'equal to'. - `<>` - 'not equal to'. It is URL-encoded (%3C%3E). - `<` - 'less than'. It is URL-encoded (%3C). - `<=` - 'less than or equal to'. It is URL-encoded (%3C=). - `>` - 'greater than'. It is URL-encoded (%3E). - `>=` - 'greater than or equal to'. It is URL-encoded (%3E=). Filters can only be applied to numeric parameters.
    ///
    /// Sets the *filters* query property to the given value.
    pub fn filters(mut self, new_value: &str) -> EntityUsageReportGetCall<'a, S> {
        self._filters = Some(new_value.to_string());
        self
    }
    /// The unique ID of the customer to retrieve data for.
    ///
    /// Sets the *customer id* query property to the given value.
    pub fn customer_id(mut self, new_value: &str) -> EntityUsageReportGetCall<'a, S> {
        self._customer_id = Some(new_value.to_string());
        self
    }
    /// The delegate implementation is consulted whenever there is an intermediate result, or if something goes wrong
    /// while executing the actual API request.
    /// 
    /// ````text
    ///                   It should be used to handle progress information, and to implement a certain level of resilience.
    /// ````
    ///
    /// Sets the *delegate* property to the given value.
    pub fn delegate(mut self, new_value: &'a mut dyn client::Delegate) -> EntityUsageReportGetCall<'a, S> {
        self._delegate = Some(new_value);
        self
    }

    /// Set any additional parameter of the query string used in the request.
    /// It should be used to set parameters which are not yet available through their own
    /// setters.
    ///
    /// Please note that this method must not be used to set any of the known parameters
    /// which have their own setter method. If done anyway, the request will fail.
    ///
    /// # Additional Parameters
    ///
    /// * *$.xgafv* (query-string) - V1 error format.
    /// * *access_token* (query-string) - OAuth access token.
    /// * *alt* (query-string) - Data format for response.
    /// * *callback* (query-string) - JSONP
    /// * *fields* (query-string) - Selector specifying which fields to include in a partial response.
    /// * *key* (query-string) - API key. Your API key identifies your project and provides you with API access, quota, and reports. Required unless you provide an OAuth 2.0 token.
    /// * *oauth_token* (query-string) - OAuth 2.0 token for the current user.
    /// * *prettyPrint* (query-boolean) - Returns response with indentations and line breaks.
    /// * *quotaUser* (query-string) - Available to use for quota purposes for server-side applications. Can be any arbitrary string assigned to a user, but should not exceed 40 characters.
    /// * *uploadType* (query-string) - Legacy upload protocol for media (e.g. "media", "multipart").
    /// * *upload_protocol* (query-string) - Upload protocol for media (e.g. "raw", "multipart").
    pub fn param<T>(mut self, name: T, value: T) -> EntityUsageReportGetCall<'a, S>
                                                        where T: AsRef<str> {
        self._additional_params.insert(name.as_ref().to_string(), value.as_ref().to_string());
        self
    }

    /// Identifies the authorization scope for the method you are building.
    ///
    /// Use this method to actively specify which scope should be used, instead of the default [`Scope`] variant
    /// [`Scope::ReportUsageReadonly`].
    ///
    /// The `scope` will be added to a set of scopes. This is important as one can maintain access
    /// tokens for more than one scope.
    ///
    /// Usually there is more than one suitable scope to authorize an operation, some of which may
    /// encompass more rights than others. For example, for listing resources, a *read-only* scope will be
    /// sufficient, a read-write scope will do as well.
    pub fn add_scope<St>(mut self, scope: St) -> EntityUsageReportGetCall<'a, S>
                                                        where St: AsRef<str> {
        self._scopes.insert(String::from(scope.as_ref()));
        self
    }
    /// Identifies the authorization scope(s) for the method you are building.
    ///
    /// See [`Self::add_scope()`] for details.
    pub fn add_scopes<I, St>(mut self, scopes: I) -> EntityUsageReportGetCall<'a, S>
                                                        where I: IntoIterator<Item = St>,
                                                         St: AsRef<str> {
        self._scopes
            .extend(scopes.into_iter().map(|s| String::from(s.as_ref())));
        self
    }

    /// Removes all scopes, and no default scope will be used either.
    /// In this case, you have to specify your API-key using the `key` parameter (see [`Self::param()`]
    /// for details).
    pub fn clear_scopes(mut self) -> EntityUsageReportGetCall<'a, S> {
        self._scopes.clear();
        self
    }
}


/// Retrieves a report which is a collection of properties and statistics for a set of users with the account. For more information, see the User Usage Report guide. For more information about the user report's parameters, see the Users Usage parameters reference guides.
///
/// A builder for the *get* method supported by a *userUsageReport* resource.
/// It is not used directly, but through a [`UserUsageReportMethods`] instance.
///
/// # Example
///
/// Instantiate a resource method builder
///
/// ```test_harness,no_run
/// # extern crate hyper;
/// # extern crate hyper_rustls;
/// # extern crate google_admin1_reports as admin1_reports;
/// # async fn dox() {
/// # use std::default::Default;
/// # use admin1_reports::{Reports, oauth2, hyper, hyper_rustls, chrono, FieldMask};
/// 
/// # let secret: oauth2::ApplicationSecret = Default::default();
/// # let auth = oauth2::InstalledFlowAuthenticator::builder(
/// #         secret,
/// #         oauth2::InstalledFlowReturnMethod::HTTPRedirect,
/// #     ).build().await.unwrap();
/// # let mut hub = Reports::new(hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().build()), auth);
/// // You can configure optional parameters by calling the respective setters at will, and
/// // execute the final call using `doit()`.
/// // Values shown here are possibly random and not representative !
/// let result = hub.user_usage_report().get("userKey", "date")
///              .parameters("vero")
///              .page_token("vero")
///              .org_unit_id("invidunt")
///              .max_results(36)
///              .group_id_filter("vero")
///              .filters("elitr")
///              .customer_id("Lorem")
///              .doit().await;
/// # }
/// ```
pub struct UserUsageReportGetCall<'a, S>
    where S: 'a {

    hub: &'a Reports<S>,
    _user_key: String,
    _date: String,
    _parameters: Option<String>,
    _page_token: Option<String>,
    _org_unit_id: Option<String>,
    _max_results: Option<u32>,
    _group_id_filter: Option<String>,
    _filters: Option<String>,
    _customer_id: Option<String>,
    _delegate: Option<&'a mut dyn client::Delegate>,
    _additional_params: HashMap<String, String>,
    _scopes: BTreeSet<String>
}

impl<'a, S> client::CallBuilder for UserUsageReportGetCall<'a, S> {}

impl<'a, S> UserUsageReportGetCall<'a, S>
where
    S: tower_service::Service<http::Uri> + Clone + Send + Sync + 'static,
    S::Response: hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{


    /// Perform the operation you have build so far.
    pub async fn doit(mut self) -> client::Result<(hyper::Response<hyper::body::Body>, UsageReports)> {
        use std::io::{Read, Seek};
        use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH, AUTHORIZATION, USER_AGENT, LOCATION};
        use client::{ToParts, url::Params};
        use std::borrow::Cow;

        let mut dd = client::DefaultDelegate;
        let mut dlg: &mut dyn client::Delegate = self._delegate.unwrap_or(&mut dd);
        dlg.begin(client::MethodInfo { id: "reports.userUsageReport.get",
                               http_method: hyper::Method::GET });

        for &field in ["alt", "userKey", "date", "parameters", "pageToken", "orgUnitID", "maxResults", "groupIdFilter", "filters", "customerId"].iter() {
            if self._additional_params.contains_key(field) {
                dlg.finished(false);
                return Err(client::Error::FieldClash(field));
            }
        }

        let mut params = Params::with_capacity(11 + self._additional_params.len());
        params.push("userKey", self._user_key);
        params.push("date", self._date);
        if let Some(value) = self._parameters.as_ref() {
            params.push("parameters", value);
        }
        if let Some(value) = self._page_token.as_ref() {
            params.push("pageToken", value);
        }
        if let Some(value) = self._org_unit_id.as_ref() {
            params.push("orgUnitID", value);
        }
        if let Some(value) = self._max_results.as_ref() {
            params.push("maxResults", value.to_string());
        }
        if let Some(value) = self._group_id_filter.as_ref() {
            params.push("groupIdFilter", value);
        }
        if let Some(value) = self._filters.as_ref() {
            params.push("filters", value);
        }
        if let Some(value) = self._customer_id.as_ref() {
            params.push("customerId", value);
        }

        params.extend(self._additional_params.iter());

        params.push("alt", "json");
        let mut url = self.hub._base_url.clone() + "admin/reports/v1/usage/users/{userKey}/dates/{date}";
        if self._scopes.is_empty() {
            self._scopes.insert(Scope::ReportUsageReadonly.as_ref().to_string());
        }

        for &(find_this, param_name) in [("{userKey}", "userKey"), ("{date}", "date")].iter() {
            url = params.uri_replacement(url, param_name, find_this, false);
        }
        {
            let to_remove = ["date", "userKey"];
            params.remove_params(&to_remove);
        }

        let url = params.parse_with_url(&url);



        loop {
            let token = match self.hub.auth.get_token(&self._scopes.iter().map(String::as_str).collect::<Vec<_>>()[..]).await {
                Ok(token) => token,
                Err(e) => {
                    match dlg.token(e) {
                        Ok(token) => token,
                        Err(e) => {
                            dlg.finished(false);
                            return Err(client::Error::MissingToken(e));
                        }
                    }
                }
            };
            let mut req_result = {
                let client = &self.hub.client;
                dlg.pre_request();
                let mut req_builder = hyper::Request::builder()
                    .method(hyper::Method::GET)
                    .uri(url.as_str())
                    .header(USER_AGENT, self.hub._user_agent.clone());

                if let Some(token) = token.as_ref() {
                    req_builder = req_builder.header(AUTHORIZATION, format!("Bearer {}", token));
                }


                        let request = req_builder
                        .body(hyper::body::Body::empty());

                client.request(request.unwrap()).await

            };

            match req_result {
                Err(err) => {
                    if let client::Retry::After(d) = dlg.http_error(&err) {
                        sleep(d).await;
                        continue;
                    }
                    dlg.finished(false);
                    return Err(client::Error::HttpError(err))
                }
                Ok(mut res) => {
                    if !res.status().is_success() {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;
                        let (parts, _) = res.into_parts();
                        let body = hyper::Body::from(res_body_string.clone());
                        let restored_response = hyper::Response::from_parts(parts, body);

                        let server_response = json::from_str::<serde_json::Value>(&res_body_string).ok();

                        if let client::Retry::After(d) = dlg.http_failure(&restored_response, server_response.clone()) {
                            sleep(d).await;
                            continue;
                        }

                        dlg.finished(false);

                        return match server_response {
                            Some(error_value) => Err(client::Error::BadRequest(error_value)),
                            None => Err(client::Error::Failure(restored_response)),
                        }
                    }
                    let result_value = {
                        let res_body_string = client::get_body_as_string(res.body_mut()).await;

                        match json::from_str(&res_body_string) {
                            Ok(decoded) => (res, decoded),
                            Err(err) => {
                                dlg.response_json_decode_error(&res_body_string, &err);
                                return Err(client::Error::JsonDecodeError(res_body_string, err));
                            }
                        }
                    };

                    dlg.finished(true);
                    return Ok(result_value)
                }
            }
        }
    }


    /// Represents the profile ID or the user email for which the data should be filtered. Can be `all` for all information, or `userKey` for a user's unique Google Workspace profile ID or their primary email address. Must not be a deleted user. For a deleted user, call `users.list` in Directory API with `showDeleted=true`, then use the returned `ID` as the `userKey`.
    ///
    /// Sets the *user key* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn user_key(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._user_key = new_value.to_string();
        self
    }
    /// Represents the date the usage occurred, based on GMT-7:00 (Pacific Standard Time). The timestamp is in the [ISO 8601 format](https://en.wikipedia.org/wiki/ISO_8601), `yyyy-mm-dd`.
    ///
    /// Sets the *date* path property to the given value.
    ///
    /// Even though the property as already been set when instantiating this call,
    /// we provide this method for API completeness.
    pub fn date(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._date = new_value.to_string();
        self
    }
    /// The `parameters` query string is a comma-separated list of event parameters that refine a report's results. The parameter is associated with a specific application. The application values for the Customers Usage report include `accounts`, `app_maker`, `apps_scripts`, `calendar`, `classroom`, `cros`, `docs`, `gmail`, `gplus`, `device_management`, `meet`, and `sites`. A `parameters` query string is in the CSV form of `app_name1:param_name1, app_name2:param_name2`. *Note:* The API doesn't accept multiple values of a parameter. If a particular parameter is supplied more than once in the API request, the API only accepts the last value of that request parameter. In addition, if an invalid request parameter is supplied in the API request, the API ignores that request parameter and returns the response corresponding to the remaining valid request parameters. An example of an invalid request parameter is one that does not belong to the application. If no parameters are requested, all parameters are returned. 
    ///
    /// Sets the *parameters* query property to the given value.
    pub fn parameters(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._parameters = Some(new_value.to_string());
        self
    }
    /// Token to specify next page. A report with multiple pages has a `nextPageToken` property in the response. In your follow-on request getting the next page of the report, enter the `nextPageToken` value in the `pageToken` query string.
    ///
    /// Sets the *page token* query property to the given value.
    pub fn page_token(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._page_token = Some(new_value.to_string());
        self
    }
    /// ID of the organizational unit to report on. User activity will be shown only for users who belong to the specified organizational unit. Data before Dec 17, 2018 doesn't appear in the filtered results.
    ///
    /// Sets the *org unit id* query property to the given value.
    pub fn org_unit_id(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._org_unit_id = Some(new_value.to_string());
        self
    }
    /// Determines how many activity records are shown on each response page. For example, if the request sets `maxResults=1` and the report has two activities, the report has two pages. The response's `nextPageToken` property has the token to the second page. The `maxResults` query string is optional.
    ///
    /// Sets the *max results* query property to the given value.
    pub fn max_results(mut self, new_value: u32) -> UserUsageReportGetCall<'a, S> {
        self._max_results = Some(new_value);
        self
    }
    /// Comma separated group ids (obfuscated) on which user activities are filtered, i.e. the response will contain activities for only those users that are a part of at least one of the group ids mentioned here. Format: "id:abc123,id:xyz456"
    ///
    /// Sets the *group id filter* query property to the given value.
    pub fn group_id_filter(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._group_id_filter = Some(new_value.to_string());
        self
    }
    /// The `filters` query string is a comma-separated list of an application's event parameters where the parameter's value is manipulated by a relational operator. The `filters` query string includes the name of the application whose usage is returned in the report. The application values for the Users Usage Report include `accounts`, `docs`, and `gmail`. Filters are in the form `[application name]:parameter name[parameter value],...`. In this example, the `<>` 'not equal to' operator is URL-encoded in the request's query string (%3C%3E): GET https://www.googleapis.com/admin/reports/v1/usage/users/all/dates/2013-03-03 ?parameters=accounts:last_login_time &filters=accounts:last_login_time%3C%3E2010-10-28T10:26:35.000Z The relational operators include: - `==` - 'equal to'. - `<>` - 'not equal to'. It is URL-encoded (%3C%3E). - `<` - 'less than'. It is URL-encoded (%3C). - `<=` - 'less than or equal to'. It is URL-encoded (%3C=). - `>` - 'greater than'. It is URL-encoded (%3E). - `>=` - 'greater than or equal to'. It is URL-encoded (%3E=). 
    ///
    /// Sets the *filters* query property to the given value.
    pub fn filters(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._filters = Some(new_value.to_string());
        self
    }
    /// The unique ID of the customer to retrieve data for.
    ///
    /// Sets the *customer id* query property to the given value.
    pub fn customer_id(mut self, new_value: &str) -> UserUsageReportGetCall<'a, S> {
        self._customer_id = Some(new_value.to_string());
        self
    }
    /// The delegate implementation is consulted whenever there is an intermediate result, or if something goes wrong
    /// while executing the actual API request.
    /// 
    /// ````text
    ///                   It should be used to handle progress information, and to implement a certain level of resilience.
    /// ````
    ///
    /// Sets the *delegate* property to the given value.
    pub fn delegate(mut self, new_value: &'a mut dyn client::Delegate) -> UserUsageReportGetCall<'a, S> {
        self._delegate = Some(new_value);
        self
    }

    /// Set any additional parameter of the query string used in the request.
    /// It should be used to set parameters which are not yet available through their own
    /// setters.
    ///
    /// Please note that this method must not be used to set any of the known parameters
    /// which have their own setter method. If done anyway, the request will fail.
    ///
    /// # Additional Parameters
    ///
    /// * *$.xgafv* (query-string) - V1 error format.
    /// * *access_token* (query-string) - OAuth access token.
    /// * *alt* (query-string) - Data format for response.
    /// * *callback* (query-string) - JSONP
    /// * *fields* (query-string) - Selector specifying which fields to include in a partial response.
    /// * *key* (query-string) - API key. Your API key identifies your project and provides you with API access, quota, and reports. Required unless you provide an OAuth 2.0 token.
    /// * *oauth_token* (query-string) - OAuth 2.0 token for the current user.
    /// * *prettyPrint* (query-boolean) - Returns response with indentations and line breaks.
    /// * *quotaUser* (query-string) - Available to use for quota purposes for server-side applications. Can be any arbitrary string assigned to a user, but should not exceed 40 characters.
    /// * *uploadType* (query-string) - Legacy upload protocol for media (e.g. "media", "multipart").
    /// * *upload_protocol* (query-string) - Upload protocol for media (e.g. "raw", "multipart").
    pub fn param<T>(mut self, name: T, value: T) -> UserUsageReportGetCall<'a, S>
                                                        where T: AsRef<str> {
        self._additional_params.insert(name.as_ref().to_string(), value.as_ref().to_string());
        self
    }

    /// Identifies the authorization scope for the method you are building.
    ///
    /// Use this method to actively specify which scope should be used, instead of the default [`Scope`] variant
    /// [`Scope::ReportUsageReadonly`].
    ///
    /// The `scope` will be added to a set of scopes. This is important as one can maintain access
    /// tokens for more than one scope.
    ///
    /// Usually there is more than one suitable scope to authorize an operation, some of which may
    /// encompass more rights than others. For example, for listing resources, a *read-only* scope will be
    /// sufficient, a read-write scope will do as well.
    pub fn add_scope<St>(mut self, scope: St) -> UserUsageReportGetCall<'a, S>
                                                        where St: AsRef<str> {
        self._scopes.insert(String::from(scope.as_ref()));
        self
    }
    /// Identifies the authorization scope(s) for the method you are building.
    ///
    /// See [`Self::add_scope()`] for details.
    pub fn add_scopes<I, St>(mut self, scopes: I) -> UserUsageReportGetCall<'a, S>
                                                        where I: IntoIterator<Item = St>,
                                                         St: AsRef<str> {
        self._scopes
            .extend(scopes.into_iter().map(|s| String::from(s.as_ref())));
        self
    }

    /// Removes all scopes, and no default scope will be used either.
    /// In this case, you have to specify your API-key using the `key` parameter (see [`Self::param()`]
    /// for details).
    pub fn clear_scopes(mut self) -> UserUsageReportGetCall<'a, S> {
        self._scopes.clear();
        self
    }
}


