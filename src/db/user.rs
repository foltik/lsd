use crate::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub version: i64,
    pub roles: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct CreateUser {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct UpdateUser {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
}

#[macro_export]
macro_rules! map_row {
    ($row:expr) => {
        User {
            id: $row.id,
            email: $row.email,
            first_name: $row.first_name,
            last_name: $row.last_name,
            phone: $row.phone,
            created_at: $row.created_at,
            updated_at: $row.updated_at,

            version: $row.version,
            roles: $row.roles.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        }
    };
}
pub use map_row;

macro_rules! map_row_fuck {
    ($row:expr) => {
        User {
            id: $row.id.unwrap(),
            email: $row.email.unwrap(),
            first_name: $row.first_name,
            last_name: $row.last_name,
            phone: $row.phone,
            created_at: $row.created_at.unwrap(),
            updated_at: $row.updated_at.unwrap(),

            version: $row.version,
            roles: $row.roles.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        }
    };
}

impl User {
    /// Full access to everything.
    pub const ADMIN: &'static str = "admin";
    /// Can manage posts.
    pub const WRITER: &'static str = "writer";

    pub async fn get_or_create(db: &Db, info: &CreateUser) -> Result<User> {
        Ok(match Self::lookup_by_email(db, &info.email).await? {
            Some(user) => user,
            None => Self::create(db, info).await?,
        })
    }

    pub async fn update_or_create(db: &Db, info: &CreateUser) -> Result<User> {
        Ok(match Self::lookup_by_email(db, &info.email).await? {
            Some(user) => {
                user.update(
                    db,
                    &UpdateUser {
                        email: info.email.clone(),
                        first_name: info.first_name.clone(),
                        last_name: info.last_name.clone(),
                        phone: info.phone.clone(),
                    },
                )
                .await?
            }
            None => Self::create(db, info).await?,
        })
    }

    /// Create a new user.
    pub async fn create(db: &Db, user: &CreateUser) -> Result<User> {
        let row = sqlx::query!(
            r#"INSERT INTO users
               (email, first_name, last_name, phone)
               VALUES (?, ?, ?, ?)
               RETURNING *, 0 as version, '' as roles
            "#,
            user.email,
            user.first_name,
            user.last_name,
            user.phone
        )
        .fetch_one(db)
        .await?;

        sqlx::query!(
            r#"INSERT INTO user_history (user_id, version, email, first_name, last_name, phone, created_at)
               VALUES (?, 0, ?, ?, ?, ?, ?)"#,
            row.id,
            user.email,
            user.first_name,
            user.last_name,
            user.phone,
            row.created_at
        )
        .execute(db)
        .await?;

        Ok(map_row!(row))
    }

    pub async fn update(&self, db: &Db, info: &UpdateUser) -> Result<Self> {
        let unchanged = info.first_name == self.first_name
            && info.last_name == self.last_name
            && info.phone == self.phone;
        if unchanged {
            return Ok(self.clone());
        }

        let new_version = self.version + 1;

        sqlx::query!(
            r#"INSERT INTO user_history (user_id, version, email, first_name, last_name, phone)
               VALUES (?, ?, ?, ?, ?, ?)"#,
            self.id,
            new_version,
            info.email,
            info.first_name,
            info.last_name,
            info.phone,
        )
        .execute(db)
        .await?;

        sqlx::query!(
            r#"UPDATE users
               SET email = ?,
                   first_name = ?,
                   last_name = ?,
                   phone = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            info.email,
            info.first_name,
            info.last_name,
            info.phone,
            self.id
        )
        .execute(db)
        .await?;

        // Get new version
        Ok(Self::lookup_by_id(db, self.id).await?.unwrap())
    }

    // pub async fn add_role(db: &Db, user_id: i64, role: &str) -> Result<()> {
    //     sqlx::query!(r#"INSERT INTO user_roles (user_id, role) VALUES (?, ?)"#, user_id, role)
    //         .execute(db)
    //         .await?;
    //     Ok(())
    // }

    /// Lookup a user by id, if one exists.
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN user_roles r ON r.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            WHERE u.id = ?
            GROUP BY u.id
            "#,
            id
        )
        .fetch_optional(db)
        .await?;
        Ok(row.map(|r| map_row_fuck!(r)))
    }

    /// Lookup a user by email address, if one exists.
    pub async fn lookup_by_email(db: &Db, email: &str) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN user_roles r ON r.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            WHERE u.email = ?
            GROUP BY u.id
            "#,
            email
        )
        .fetch_optional(db)
        .await?;
        Ok(row.map(|r| map_row!(r)))
    }
    /// Lookup a user by a login token, if it's valid.
    pub async fn lookup_by_login_token(db: &Db, token: &str) -> Result<Option<User>> {
        // Weird workaround for sqlx incorrectly inferring nullability for joins
        // not sure why this is needed here and not below
        // use the "!" syntax to force the column to be interpreted as non-null
        // https://github.com/launchbadge/sqlx/issues/2127
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN login_tokens t ON t.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            LEFT JOIN user_roles r ON r.user_id = u.id
            WHERE t.token = ?
            GROUP BY u.id"#,
            token
        )
        .fetch_optional(db)
        .await?;

        Ok(row.map(|r| map_row_fuck!(r)))
    }
    /// Lookup a user by a session token, if it's valid.
    pub async fn lookup_by_session_token(db: &Db, token: &str) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN session_tokens t ON t.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            LEFT JOIN user_roles r ON r.user_id = u.id
            WHERE t.token = ?
            GROUP BY u.id
            "#,
            token
        )
        .fetch_optional(db)
        .await?;
        Ok(row.map(|r| map_row!(r)))
    }

    pub async fn lookup_by_list_id(db: &Db, list_id: i64) -> Result<Vec<User>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM list_members lm
            JOIN users u ON u.id = lm.user_id
            JOIN user_history h ON h.user_id = u.id
            LEFT JOIN user_roles r ON r.user_id = u.id
            WHERE lm.list_id = ?
            GROUP BY u.id
            "#,
            list_id,
        )
        .fetch_all(db)
        .await?;
        Ok(rows.into_iter().map(|r| map_row!(r)).collect())
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn has_staff_role(&self) -> bool {
        self.roles.iter().any(|r| [Self::ADMIN, Self::WRITER].contains(&&**r))
    }
}
