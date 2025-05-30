macro_rules! perm_check {
    ($conn:expr, $initiator:expr, $journal:expr, $scope:expr, $ability:expr) => {
        let perm_check = if $journal.users_id == $initiator.user.id {
            crate::sec::authz::has_permission($conn, $initiator.user.id, $scope, $ability)
                .await
                .context("failed to retrieve permissiosn for user")?
        } else {
            crate::sec::authz::has_permission_ref(
                $conn,
                $initiator.user.id,
                $scope,
                $ability,
                $journal.id,
            )
            .await
            .context("failed to retrieve permissions for user")?
        };

        if !perm_check {
            return Ok(axum::http::StatusCode::UNAUTHORIZED.into_response());
        }
    };
}

pub(crate) use perm_check;
