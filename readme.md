# Rinha de Backend 2025 - Implementação em Rust

Esta é uma implementação para a terceira edição da Rinha de Backend, desenvolvida em Rust com foco em alta performance, resiliência e baixo consumo de recursos.

## Tecnologias Utilizadas

* **Linguagem:** Rust (stable)
* **Framework Web:** Axum
* **Runtime Assíncrono:** Tokio
* **Banco de Dados:** Redis (para persistência e sumários pré-agregados)
* **Mensageria:**
    * **NATS:** Usado para a comunicação de status de saúde entre as instâncias da API (Líder e Colaboradora).
    * **Tokio MPSC:** Usado como a fila de trabalho interna para desacoplar o recebimento de requisições do processamento.
* **Load Balancer:** Nginx (padrão) / HAProxy (alternativa)
* **Serialização:** `simd-json` para desserialização de alta performance.
* **Alocador de Memória:** `jemalloc` para gerenciamento de memória mais eficiente em ambiente multi-threaded.
* **Containerização:** Docker & Docker Compose

## Arquitetura Geral

A solução foi projetada para ser horizontalmente escalável e resiliente, seguindo as especificações do desafio.

1.  **Load Balancer:** Um **Nginx** (ou **HAProxy**) atua como a porta de entrada, recebendo todo o tráfego na porta `9999` e distribuindo a carga entre duas instâncias da API.
2.  **API (Rust/Axum):** Duas instâncias da aplicação rodam em contêineres separados.
    * A instância **LÍDER** é responsável por realizar os *health checks* periódicos nos processadores de pagamento externos e transmitir o status via NATS.
    * A instância **COLABORADORA** (e também a LÍDER) escuta as mensagens de status no NATS para manter seu estado interno sobre a saúde dos processadores sempre atualizado.
3.  **Fila de Trabalho:** O endpoint `POST /payments` é extremamente rápido. Ele apenas valida a requisição e a envia para uma fila de trabalho interna (MPSC), respondendo `200 OK` imediatamente.
4.  **Workers:** Um pool de workers (tarefas Tokio) consome os pagamentos da fila em background. É aqui que toda a lógica de negócio acontece: escolher o melhor processador, fazer a chamada HTTP, tratar falhas e retentativas.
5.  **Persistência (Redis):** Após um pagamento ser processado com sucesso, o worker o salva no Redis. A persistência é otimizada usando duas estratégias:
    * **Dados Individuais:** Cada pagamento é salvo com um índice de tempo de alta precisão (microssegundos) para permitir consultas exatas.
    * **Sumários Pré-agregados:** Na mesma transação, contadores para o sumário daquele **segundo** específico são incrementados, tornando a consulta `GET /payments-summary` quase instantânea.

## Explicação das Branches

Este repositório contém diferentes estratégias de implementação, cada uma em sua branch, para explorar os trade-offs de concorrência e infraestrutura.

### 🌳 `round_robin` (Branch Principal)
Esta é a implementação principal e a mais performática, utilizada na imagem Docker final.

* **Estratégia:** Elimina completamente a contenção de locks na fila de trabalho.
* **Como Funciona:** Em vez de uma única fila compartilhada, cada worker possui seu próprio canal MPSC dedicado. O handler `submit_work_handler` atua como um dispatcher: ele usa um contador atômico para distribuir as requisições de pagamento recebidas entre os canais dos workers em um padrão **Round-Robin**.
* **Vantagem:** Permite que todos os workers fiquem 100% paralelos, sem nunca precisarem esperar um pelo outro para pegar um novo trabalho da fila. É a arquitetura de maior vazão (throughput).

### 🌳 `haproxy`
Esta branch é funcionalmente idêntica à `round_robin`, com uma única mudança na infraestrutura.

* **Estratégia:** Substituir o Nginx pelo HAProxy como load balancer.
* **Como Funciona:** O arquivo `docker-compose.yaml` foi modificado para usar o serviço do `haproxy` em vez do `nginx`. Um arquivo `haproxy.cfg` foi adicionado com otimizações de performance e health checks ativos.
* **Vantagem:** O HAProxy oferece health checks mais robustos e proativos, podendo parar de enviar tráfego para uma instância da API que travou, o que aumenta a resiliência geral do sistema. OBS: Não utilizo o health check

### 🌳 `pool`
Esta branch implementa uma estratégia de concorrência mais simples, mas com um ponto de contenção.

* **Estratégia:** Todos os workers compartilham um único receptor da fila.
* **Como Funciona:** Existe apenas um canal MPSC. O lado do receptor (`Receiver`) é envolvido em um `Arc<Mutex<...>>`. Todos os workers, em um loop, competem para obter o `lock` do Mutex e ter a chance de chamar `.recv().await` para pegar o próximo pagamento da fila.
* **Vantagem:** Código de inicialização um pouco mais simples.
* **Desvantagem:** O `Mutex` se torna um gargalo. Apenas um worker pode estar esperando por um item da fila por vez, o que limita a escalabilidade e a vazão em comparação com a abordagem `round_robin`.