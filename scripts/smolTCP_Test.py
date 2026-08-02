import socket
import time
import random
import string
import statistics
import sys
from dataclasses import dataclass

# ==========================================
# Configurações Globais
# ==========================================
HOST = '10.0.0.2'
PORT = 5555
TEMPO_TESTE = 10         # Duração do teste em segundos
TIMEOUT_RECV = 1.0       # Segundos aguardando resposta antes de considerar pacote perdido
REPETICOES_POR_TESTE = 5

TEST_PROFILES = [
    ("Latência", 50, 64),
    ("Throughput", 500, 1460),
    ("Memory Reassemble", 200, 8192),
    ("Interruption Stress", 15000, 64),
]


@dataclass
class TestResult:
    protocolo: str
    perfil: str
    taxa_poisson: int
    tamanho_pacote: int
    duracao_real: float
    pacotes_enviados: int
    pacotes_recebidos: int
    pacotes_validos: int
    erros_validacao: int
    pacotes_perdidos: int
    throughput_mbps: float
    latencia_media_ms: float
    jitter_ms: float
    erro: str | None = None

def gerar_payload(tamanho):
    """Gera uma string aleatória de letras e números."""
    caracteres = string.ascii_letters + string.digits
    return ''.join(random.choices(caracteres, k=tamanho))

def run_test(nome_perfil, taxa_poisson, tamanho_pacote, usar_udp, tcp_socket=None):
    protocolo_str = "UDP" if usar_udp else "TCP"

    if usar_udp:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.settimeout(TIMEOUT_RECV)
            return executar_teste_no_socket(
                s,
                nome_perfil,
                taxa_poisson,
                tamanho_pacote,
                usar_udp,
                protocolo_str,
            )

    if tcp_socket is None:
        return criar_resultado_erro(
            protocolo_str,
            nome_perfil,
            taxa_poisson,
            tamanho_pacote,
            "Socket TCP não inicializado.",
        )

    return executar_teste_no_socket(
        tcp_socket,
        nome_perfil,
        taxa_poisson,
        tamanho_pacote,
        usar_udp,
        protocolo_str,
    )


def executar_teste_no_socket(s, nome_perfil, taxa_poisson, tamanho_pacote, usar_udp, protocolo_str):
    latencias = []
    total_bytes_enviados = 0
    pacotes_enviados = 0
    pacotes_perdidos = 0
    sucessos_validacao = 0
    erros_validacao = 0

    tempo_inicio = time.time()

    try:
        s.settimeout(TIMEOUT_RECV)
        while (time.time() - tempo_inicio) < TEMPO_TESTE:
            payload_str = gerar_payload(tamanho_pacote)
            if not usar_udp:
                payload_str += '\n'
            payload_bytes = payload_str.encode('utf-8')

            t_envio = time.perf_counter()

            try:
                if usar_udp:
                    s.sendto(payload_bytes, (HOST, PORT))
                else:
                    s.sendall(payload_bytes)

                total_bytes_enviados += len(payload_bytes)
                pacotes_enviados += 1

                dados_recebidos = b''

                if usar_udp:
                    dados_recebidos, _ = s.recvfrom(tamanho_pacote + 512)
                else:
                    while len(dados_recebidos) < len(payload_bytes):
                        parte = s.recv(len(payload_bytes) - len(dados_recebidos))
                        if not parte:
                            raise ConnectionError("Conexão TCP encerrada pelo servidor.")
                        dados_recebidos += parte

                t_recebimento = time.perf_counter()

                rtt_ms = (t_recebimento - t_envio) * 1000
                latencias.append(rtt_ms)

                resposta_str = dados_recebidos.decode('utf-8')
                if not usar_udp and resposta_str.endswith('\n'):
                    resposta_str = resposta_str[:-1]
                    payload_str = payload_str[:-1]

                if resposta_str == payload_str[::-1]:
                    sucessos_validacao += 1
                else:
                    erros_validacao += 1

            except socket.timeout:
                pacotes_perdidos += 1

            tempo_espera = random.expovariate(taxa_poisson)
            time.sleep(tempo_espera)

    except Exception as e:
        return criar_resultado_erro(
            protocolo_str,
            nome_perfil,
            taxa_poisson,
            tamanho_pacote,
            f"Erro durante a comunicação: {e}",
        )

    tempo_fim = time.time()
    duracao_real = tempo_fim - tempo_inicio

    if duracao_real > 0:
        throughput_bps = (total_bytes_enviados * 8) / duracao_real
        throughput_mbps = throughput_bps / 1_000_000
    else:
        throughput_mbps = 0

    if latencias:
        latencia_media_ms = statistics.mean(latencias)
        jitter_ms = statistics.stdev(latencias) if len(latencias) > 1 else 0
    else:
        latencia_media_ms = 0
        jitter_ms = 0

    return TestResult(
        protocolo=protocolo_str,
        perfil=nome_perfil,
        taxa_poisson=taxa_poisson,
        tamanho_pacote=tamanho_pacote,
        duracao_real=duracao_real,
        pacotes_enviados=pacotes_enviados,
        pacotes_recebidos=len(latencias),
        pacotes_validos=sucessos_validacao,
        erros_validacao=erros_validacao,
        pacotes_perdidos=pacotes_perdidos,
        throughput_mbps=throughput_mbps,
        latencia_media_ms=latencia_media_ms,
        jitter_ms=jitter_ms,
    )


def criar_resultado_erro(protocolo, perfil, taxa_poisson, tamanho_pacote, erro):
    return TestResult(
        protocolo=protocolo,
        perfil=perfil,
        taxa_poisson=taxa_poisson,
        tamanho_pacote=tamanho_pacote,
        duracao_real=0,
        pacotes_enviados=0,
        pacotes_recebidos=0,
        pacotes_validos=0,
        erros_validacao=0,
        pacotes_perdidos=0,
        throughput_mbps=0,
        latencia_media_ms=0,
        jitter_ms=0,
        erro=erro,
    )


def media(resultados, atributo):
    return statistics.mean(getattr(resultado, atributo) for resultado in resultados)


def imprimir_resultado_agregado(protocolo, perfil, taxa_poisson, tamanho_pacote, resultados):
    execucoes_validas = [resultado for resultado in resultados if resultado.erro is None]
    execucoes_com_erro = [resultado for resultado in resultados if resultado.erro is not None]

    print("\n" + "=" * 78)
    print(f"{protocolo} | {perfil} | λ={taxa_poisson} pct/s | Tamanho={tamanho_pacote} bytes")
    print("=" * 78)

    if not execucoes_validas:
        print("Nenhuma execução concluída com sucesso.")
        for indice, resultado in enumerate(resultados, start=1):
            print(f"Execução {indice}: {resultado.erro}")
        return

    total_recebidos = sum(resultado.pacotes_recebidos for resultado in execucoes_validas)
    total_validos = sum(resultado.pacotes_validos for resultado in execucoes_validas)
    taxa_sucesso_validacao = (total_validos / total_recebidos) * 100 if total_recebidos else 0

    print(f"Execuções concluídas:          {len(execucoes_validas)}/{len(resultados)}")
    if execucoes_com_erro:
        print(f"Execuções com erro:            {len(execucoes_com_erro)}")
    print(f"Pacotes enviados (média):      {media(execucoes_validas, 'pacotes_enviados'):.2f}")
    print(f"Pacotes recebidos (média):     {media(execucoes_validas, 'pacotes_recebidos'):.2f}")
    print(f"Pacotes válidos/recebidos:     {media(execucoes_validas, 'pacotes_validos'):.2f}/{media(execucoes_validas, 'pacotes_recebidos'):.2f}")
    print(f"Taxa de sucesso validação:     {taxa_sucesso_validacao:.2f}%")
    print(f"Throughput médio:              {media(execucoes_validas, 'throughput_mbps'):.4f} Mbps")
    print(f"Latência média:                {media(execucoes_validas, 'latencia_media_ms'):.2f} ms")
    print(f"Jitter médio:                  {media(execucoes_validas, 'jitter_ms'):.2f} ms")

def main():
    print("\n" + "=" * 78)
    print("SUÍTE AUTOMÁTICA DE TESTES DE REDE")
    print(f"Destino: {HOST}:{PORT} | Duração por execução: {TEMPO_TESTE}s | Repetições: {REPETICOES_POR_TESTE}")
    print("=" * 78)

    for usar_udp in (False, True):
        protocolo = "UDP" if usar_udp else "TCP"
        tcp_socket = None
        erro_conexao_tcp = None

        if not usar_udp:
            try:
                tcp_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                tcp_socket.settimeout(TIMEOUT_RECV)
                tcp_socket.connect((HOST, PORT))
            except ConnectionRefusedError:
                erro_conexao_tcp = "Não foi possível conectar. O servidor está rodando?"
            except Exception as e:
                erro_conexao_tcp = f"Erro ao abrir conexão TCP: {e}"

        try:
            for nome_perfil, taxa, tamanho in TEST_PROFILES:
                resultados = []
                for repeticao in range(1, REPETICOES_POR_TESTE + 1):
                    print(
                        f"\rExecutando {protocolo} / {nome_perfil}: {repeticao}/{REPETICOES_POR_TESTE}",
                        end="",
                        file=sys.stderr,
                        flush=True,
                    )

                    if erro_conexao_tcp is not None:
                        resultados.append(
                            criar_resultado_erro(
                                protocolo,
                                nome_perfil,
                                taxa,
                                tamanho,
                                erro_conexao_tcp,
                            )
                        )
                    else:
                        resultados.append(run_test(nome_perfil, taxa, tamanho, usar_udp, tcp_socket))

                print("\r" + " " * 78 + "\r", end="", file=sys.stderr)
                imprimir_resultado_agregado(protocolo, nome_perfil, taxa, tamanho, resultados)
        finally:
            if tcp_socket is not None:
                tcp_socket.close()

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\nTeste interrompido pelo usuário. Saindo...")
        sys.exit(0)
