PRAGMA writable_schema = ON;

DELETE FROM users WHERE first_name IS NULL OR last_name IS NULL;

UPDATE sqlite_master
   SET sql = replace(sql, 'first_name TEXT', 'first_name TEXT NOT NULL')
   WHERE tbl_name = 'users' AND type = 'table';
UPDATE sqlite_master
   SET sql = replace(sql, 'last_name TEXT', 'last_name TEXT NOT NULL')
   WHERE tbl_name = 'users' AND type = 'table';

PRAGMA writable_schema = OFF;
