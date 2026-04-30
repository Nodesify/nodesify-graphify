package main

import "fmt"

type Server struct {
    Port int
}

func NewServer(port int) *Server {
    return &Server{Port: port}
}

func (s *Server) Start() {
    fmt.Printf("Server started on port %d\n", s.Port)
}

func main() {
    s := NewServer(8080)
    s.Start()
}
