-- Login RPCはorg contextなしでusernameで検索するため、FORCE RLSを解除
-- postgresユーザー(table owner)はRLSをバイパスできるようになる
-- RLS自体は有効のまま（非ownerユーザーにはポリシーが適用される）
ALTER TABLE api_users NO FORCE ROW LEVEL SECURITY;
