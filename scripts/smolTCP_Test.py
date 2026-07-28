import socket
import time
import random
import string
import statistics
import sys

# ==========================================
# Configurações Globais
# ==========================================
HOST = '10.0.0.2'
PORT = 5555
TEMPO_TESTE = 10         # Duração do teste em segundos
TIMEOUT_RECV = 1.0       # Segundos aguardando resposta antes de considerar pacote perdido

def gerar_payload(tamanho):
    """Gera uma string aleatória de letras e números."""
    caracteres = string.ascii_letters + string.digits
    return ''.join(random.choices(caracteres, k=tamanho))

def run_test(taxa_poisson, tamanho_pacote, usar_udp):
    latencias = []
    total_bytes_enviados = 0
    pacotes_enviados = 0
    pacotes_perdidos = 0
    sucessos_validacao = 0
    erros_validacao = 0
    
    tipo_socket = socket.SOCK_DGRAM if usar_udp else socket.SOCK_STREAM
    protocolo_str = "UDP" if usar_udp else "TCP"
    
    with socket.socket(socket.AF_INET, tipo_socket) as s:
        s.settimeout(TIMEOUT_RECV)
        
        try:
            print(f"\n[*] Modo de Operação: {protocolo_str}")
            if not usar_udp:
                s.connect((HOST, PORT))
                print(f"[*] Conectado a {HOST}:{PORT}")
            else:
                print(f"[*] Preparado para enviar via UDP para {HOST}:{PORT}")
                
            print(f"[*] Iniciando teste de {TEMPO_TESTE} segundos (λ = {taxa_poisson} pct/s, Tamanho = {tamanho_pacote} bytes)...")
            
            tempo_inicio = time.time()
            
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
                                break # Conexão caiu
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
                
        except ConnectionRefusedError:
            print("[-] Erro: Não foi possível conectar. O servidor está rodando?")
            return
        except Exception as e:
            print(f"[-] Erro durante a comunicação: {e}")
            return

    tempo_fim = time.time()
    duracao_real = tempo_fim - tempo_inicio

    if duracao_real > 0:
        throughput_bps = (total_bytes_enviados * 8) / duracao_real
        throughput_mbps = throughput_bps / 1_000_000
    else:
        throughput_mbps = 0

    print("\n" + "="*45)
    print(f"       RESULTADOS DO TESTE ({protocolo_str})       ")
    print("="*45)
    print(f"Duração real:          {duracao_real:.2f} segundos")
    print(f"Pacotes enviados:      {pacotes_enviados}")
    if usar_udp:
        taxa_perda = (pacotes_perdidos / pacotes_enviados) * 100 if pacotes_enviados > 0 else 0
        print(f"Pacotes perdidos:      {pacotes_perdidos} ({taxa_perda:.2f}%)")
    print(f"Pacotes recebidos:     {len(latencias)}")
    print(f"Total de dados env.:   {total_bytes_enviados / 1024:.2f} KB")
    print(f"Throughput Médio:      {throughput_mbps:.4f} Mbps")
    print("-" * 45)
    
    if latencias:
        print(f"Latência Mínima:       {min(latencias):.2f} ms")
        print(f"Latência Máxima:       {max(latencias):.2f} ms")
        print(f"Latência Média:        {statistics.mean(latencias):.2f} ms")
        print(f"Jitter (Desvio Padrão): {statistics.stdev(latencias) if len(latencias) > 1 else 0:.2f} ms")
    else:
        print("Nenhum pacote retornou. Impossível calcular latência.")
        
    print("-" * 45)
    print(f"Validações Sucesso:    {sucessos_validacao}")
    print(f"Erros de Validação:    {erros_validacao}")
    print("="*45)

def main():
    while True:
        print("\n" + "="*35)
        print("        MENU DE TESTES DE REDE")
        print("="*35)
        
        print("\n1) Selecione o Protocolo:")
        print("   [1] TCP")
        print("   [2] UDP")
        print("   [0] Sair")
        
        proto_opcao = input("\nOpção: ").strip()
        
        if proto_opcao == '0':
            print("Saindo...")
            sys.exit(0)
        elif proto_opcao == '1':
            usar_udp = False
        elif proto_opcao == '2':
            usar_udp = True
        else:
            print("[-] Opção inválida. Tente novamente.")
            continue

        print("\n2) Selecione o Tipo de Teste:")
        print("   [1] Latência             (λ = 50 pct/s,    Tam = 64 bytes)")
        print("   [2] Throughput           (λ = 500 pct/s,   Tam = 1460 bytes)")
        print("   [3] Memory Reassemble    (λ = 200 pct/s,   Tam = 8192 bytes)")
        print("   [4] Interruption Stress  (λ = 15000 pct/s, Tam = 64 bytes)")
        print("   [0] Voltar")
        
        teste_opcao = input("\nOpção: ").strip()
        
        if teste_opcao == '0':
            continue
        elif teste_opcao == '1':
            taxa = 50
            tamanho = 64
        elif teste_opcao == '2':
            taxa = 500
            tamanho = 1460
        elif teste_opcao == '3':
            taxa = 200
            tamanho = 8192
        elif teste_opcao == '4':
            taxa = 15000
            tamanho = 64
        else:
            print("[-] Opção inválida. Tente novamente.")
            continue
            
        run_test(taxa_poisson=taxa, tamanho_pacote=tamanho, usar_udp=usar_udp)
        
        input("\nPressione ENTER para voltar ao menu principal...")

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\nTeste interrompido pelo usuário. Saindo...")
        sys.exit(0)
