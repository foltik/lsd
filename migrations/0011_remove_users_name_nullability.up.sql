PRAGMA writable_schema = ON;

UPDATE sqlite_master
   SET sql = replace(sql, 'first_name TEXT NOT NULL', 'first_name TEXT')
   WHERE tbl_name = 'users' AND type = 'table';
UPDATE sqlite_master
   SET sql = replace(sql, 'last_name TEXT NOT NULL', 'last_name TEXT')
   WHERE tbl_name = 'users' AND type = 'table';

PRAGMA writable_schema = OFF;
