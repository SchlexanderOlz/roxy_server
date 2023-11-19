import re
import ssl
import socket
import http.client
import threading
import subprocess


proxy_host = '0.0.0.0'
proxy_port = 8080

# Create a socket for the proxy server
proxy_server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
proxy_server.bind((proxy_host, proxy_port))
proxy_server.listen(1)


KEY_OUT = "./keys"


def generate_ca():
    subprocess.run(
        ["openssl", "genpkey", "-algorithm", "RSA", "-out", "ca.key"],
        cwd=KEY_OUT
    )

    subprocess.run(
        ["openssl", "req", "-x509", "-new", "-key", "ca.key", "-out", "ca.crt", "-subj", "/CN=RootCA"],
        cwd=KEY_OUT
    )

def generate_certificate(target: str):
    url: str = "".join(re.findall(r'[^0-9:]', target))
    print(url)
    subprocess.run(
        ["openssl", "genpkey", "-algorithm", "RSA", "-out", "{}.key".format(url)],
        cwd=KEY_OUT
    )

    subprocess.run(
        [
            "openssl",
            "req",
            "-new",
            "-key",
            "{}.key".format(url),
            "-out",
            "{}.csr".format(url),
            "-subj",
            "/CN=" + url,
        ],
        cwd=KEY_OUT
    )

    subprocess.run(
        [
            "openssl",
            "x509",
            "-req",
            "-in",
            "{}.csr".format(url),
            "-CA",
            "ca.crt",
            "-CAkey",
            "ca.key",
            "-out",
            "{}.crt".format(url),
            "-CAcreateserial"
        ],
        cwd=KEY_OUT
    )


def generate_http_request(method: str, url: str, headers=None):
    connection = http.client.HTTPConnection(url)

    request_message = http.client.HTTPMessage()
    request_message.add_header("Host", url)
    request_message.add_header("User-Agent", "MyProxy")
    request_message.add_header("Accept", "*/*")

    if headers:
        for key, value in headers.items():
            request_message.add_header(key, value)

    return request_message.as_bytes()

def handle_connect(client_socket: socket.socket, dest_header: str):
    target_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    request = dest_header.split("\r")[0].split(" ")
    dest: str = request[1]
    host: str = "".join(re.findall(r'[^0-9:]', dest))
    port: int = int(dest.replace(host, "").replace(":", ""))

    generate_certificate(dest)
    target_context = ssl.SSLContext(protocol=ssl.PROTOCOL_TLS_CLIENT)
    target_context.check_hostname = False
    target_context.verify_mode = ssl.CERT_NONE
    print("{}/{}.crt".format(KEY_OUT, host))
    target_context.load_cert_chain(certfile="{}/{}.crt".format(KEY_OUT, host), keyfile="{}/{}.key".format(KEY_OUT, host))
    target_context.load_default_certs()

    target_socket.connect((host, port))
    target_socket = target_context.wrap_socket(target_socket, server_side=False) 

    print(dest_header)
    client_socket.send(b'HTTP/1.1 200 Connection Established\r\n\r\n')

    try:
        request = generate_http_request("GET", host)
        print(request)
        target_socket.send(request)

        # Relay data between the client and target server
        while True:
            response: bytes = target_socket.recv(4096)
            # print(response.decode())
            if not response:
                print("saaaaaaaseeeenssooooss")
                break
            client_socket.send(response)

            data: bytes = client_socket.recv(4096)
            # print(data.decode())

            if not data:
                break
            target_socket.send(data)

    except KeyboardInterrupt:
        pass
    client_socket.close()
    target_socket.close()


def handle_http(client_socket: socket.socket, dest_header: str):
    print(dest_header)
    request = dest_header.split("\r")[0].split(" ")
    destination: str = request[1]
    host: str = dest_header.split("\r")[1].split(" ")[1]

    target_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    print(host)
    target_socket.connect((host, 80))
    target_socket.send(dest_header.encode())

    try:
        while True:
            msg = target_socket.recv(4096)
            if len(msg) == 0:
                return
            client_socket.send(msg)
    except KeyboardInterrupt:
        pass
    target_socket.close()
    client_socket.close()


def handle_client(client_socket: socket.socket):
    dest_header: str = client_socket.recv(4096).decode()

    request = dest_header.split("\r")[0].split(" ")
    method = request[0]

    print(method)

    if method == "CONNECT":
        handle_connect(client_socket, dest_header)
        return
    handle_http(client_socket, dest_header)


# Main proxy loop
if __name__ == "__main__":
    while True:
        print(ssl.OPENSSL_VERSION)
        client_socket, addr = proxy_server.accept()
        client_handler = threading.Thread(target=handle_client, args=(client_socket,))
        client_handler.start()
