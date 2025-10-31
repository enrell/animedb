package repository

import (
	"fmt"
	"strings"
)

type QueryBuilder struct {
	conditions []string
	args       []any
}

func NewQueryBuilder() *QueryBuilder {
	return &QueryBuilder{
		conditions: make([]string, 0),
		args:       make([]any, 0),
	}
}

func (qb *QueryBuilder) AddCondition(condition string, arg any) *QueryBuilder {
	idx := len(qb.args) + 1
	qb.conditions = append(qb.conditions, fmt.Sprintf(condition, idx))
	qb.args = append(qb.args, arg)
	return qb
}

func (qb *QueryBuilder) AddRawCondition(condition string) *QueryBuilder {
	qb.conditions = append(qb.conditions, condition)
	return qb
}

func (qb *QueryBuilder) AddArg(arg any) int {
	qb.args = append(qb.args, arg)
	return len(qb.args)
}

func (qb *QueryBuilder) BuildWhereClause() string {
	if len(qb.conditions) == 0 {
		return ""
	}
	return "WHERE " + strings.Join(qb.conditions, " AND ")
}

func (qb *QueryBuilder) Args() []any {
	return qb.args
}

func (qb *QueryBuilder) ArgCount() int {
	return len(qb.args)
}

func (qb *QueryBuilder) WithPagination(pageSize, offset int) ([]any, string) {
	args := append(qb.args, pageSize, offset)
	clause := fmt.Sprintf(" LIMIT $%d OFFSET $%d", len(args)-1, len(args))
	return args, clause
}

