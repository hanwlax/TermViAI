"""Synthetic loopback-only SSH fixture; requires paramiko. No real accounts."""
import argparse
import paramiko, socket, threading, pathlib, time, json
parser = argparse.ArgumentParser()
parser.add_argument('directory', type=pathlib.Path)
root = parser.parse_args().directory
root.mkdir(parents=True, exist_ok=True)
# Keys are regenerated on each start; remove the previous fixture fingerprint.
(root / 'known_hosts').unlink(missing_ok=True)
host=paramiko.RSAKey.generate(2048);good=paramiko.RSAKey.generate(2048);bad=paramiko.RSAKey.generate(2048)
host.write_private_key_file(str(root/'host_key'));good.write_private_key_file(str(root/'good_key'));good.write_private_key_file(str(root/'encrypted_key'),password='fixture-passphrase');bad.write_private_key_file(str(root/'bad_key'))
log=open(root/'attempts.jsonl','w',buffering=1)
class Server(paramiko.ServerInterface):
 def get_allowed_auths(self,u):return 'publickey,password'
 def check_auth_publickey(self,u,key):
  ok=u=='key-user' and key==good
  log.write(json.dumps({'method':'key','username':u,'accepted':ok})+'\n')
  return paramiko.AUTH_SUCCESSFUL if ok else paramiko.AUTH_FAILED
 def check_auth_password(self,u,p):
  ok=(u,p) in [('identity-user','identity-good'),('manual-user','manual-good')]
  log.write(json.dumps({'method':'password','username':u,'accepted':ok})+'\n')
  return paramiko.AUTH_SUCCESSFUL if ok else paramiko.AUTH_FAILED
 def check_channel_request(self,k,i):return paramiko.OPEN_SUCCEEDED if k=='session' else paramiko.OPEN_FAILED_ADMINISTRATIVELY_PROHIBITED
 def check_channel_pty_request(self,*a):return True
 def check_channel_shell_request(self,c):
  threading.Thread(target=lambda:(c.send(b'fixture-ready\r\n'),time.sleep(10),c.close()),daemon=True).start();return True
 def check_channel_exec_request(self,c,cmd):
  def answer(): c.send(b'fixture-ready\n');c.send_exit_status(0);c.close()
  threading.Thread(target=answer,daemon=True).start();return True
 def check_global_request(self,*a):return True
server=socket.socket();server.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);server.bind(('127.0.0.1',22371));server.listen(20)
print('Synthetic SSH fixture ready on 127.0.0.1:22371',flush=True)
def handle(sock):
 t=paramiko.Transport(sock);t.add_server_key(host)
 try:
  t.start_server(server=Server())
  while t.is_active():time.sleep(.1)
 except Exception as e:pass
 finally:t.close()
while True:
 sock,_=server.accept();threading.Thread(target=handle,args=(sock,),daemon=True).start()
