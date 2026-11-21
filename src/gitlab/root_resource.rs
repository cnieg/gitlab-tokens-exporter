//! A trait implemented by GitLab root resources.
///
/// Root resources are top-level GitLab resources that can be retrieved
/// independently, without requiring a parent resource. These include
/// [projects](crate::gitlab::project::Project),
/// [groups](crate::gitlab::group::Group),
/// [users](crate::gitlab::user::User), and
/// [personal access tokens](crate::gitlab::token::personal::PersonalAccessToken).
///
/// Implementors provide the URL used to retrieve the resource collection
/// and a human-readable name identifying the resource type.
pub trait GitlabRootResource {
    /// Returns the URL of the GitLab API endpoint for this resource type.
    fn first_url() -> String;

    /// Returns the name of this resource type.
    ///
    /// This name is typically used in logs and error messages.
    fn resource_type_name() -> &'static str;
}
